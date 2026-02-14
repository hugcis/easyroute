# Mobile App: Embedded Server + WebView

This document describes the path from the current `ondevice` binary to a native iOS/Android app using the embedded server + WebView approach.

## Architecture

```
┌─────────────────────────────────────┐
│  iOS/Android App                    │
│                                     │
│  ┌──────────────┐  ┌────────────┐  │
│  │ Background   │  │ WKWebView  │  │
│  │ Thread:      │  │ / WebView  │  │
│  │ Tokio +      │◄─┤            │  │
│  │ Axum on      │  │ app/ UI    │  │
│  │ 127.0.0.1    │  │ (embedded) │  │
│  └──────┬───────┘  └────────────┘  │
│         │                           │
│  ┌──────┴───────┐                   │
│  │ SQLite .db   │                   │
│  │ (app data)   │                   │
│  └──────────────┘                   │
└─────────────────┬───────────────────┘
                  │ HTTPS
                  ▼
           Mapbox Proxy (fly.io)
```

The app is a thin native shell (Swift or Kotlin) that does three things:

1. Starts the Rust server on a background thread, bound to `127.0.0.1`
2. Opens a full-screen WebView pointing at `http://localhost:{port}`
3. Manages the region `.db` file (bundled or downloaded)

All route generation, POI queries, caching, and Mapbox communication happen inside the Rust process — the same code that runs in the `ondevice` binary today.

## Why This Approach

**Reuse over rewrite.** The `ondevice` binary already does everything a mobile app needs: SQLite queries, route generation, Mapbox API calls, and serving a web UI. Wrapping it in a native shell is the shortest path to a working app.

**The web UI already exists.** The `app/` directory (index.html, style.css, main.js) renders routes on a Mapbox GL map, handles POI categories, exports to GPX/GeoJSON, and supports geolocation. All of this works in a mobile WebView without changes.

**No new dependencies.** The Rust core compiles to iOS and Android targets today — SQLite is native on both platforms, reqwest uses platform TLS, and Tokio runs on any OS with threads.

**Incremental path to native UI.** If the WebView UX proves insufficient, individual screens can be replaced with native views (SwiftUI / Jetpack Compose) one at a time. The Rust server doesn't change — the native UI just calls the same localhost API.

## What Works Today, Unchanged

| Component | Mobile status |
|-----------|--------------|
| `RouteGenerator` (scoring, waypoints, metrics, geometry) | Pure Rust, no platform deps |
| `SqlitePoiRepository` (R-tree spatial queries) | SQLite is native on iOS/Android |
| `MemoryCacheService` (moka) | Pure Rust |
| `MapboxClient` (reqwest) | Uses platform TLS via `native-tls` |
| `app/` web UI | Works in WKWebView / Android WebView |
| Mapbox proxy communication | Standard HTTPS |

## Implementation Steps

### Step 1: Embed Static Assets in the Binary

Currently `ondevice` serves files from the `app/` directory on disk via `ServeDir`. For mobile, the assets need to be compiled into the binary so there's no filesystem dependency.

Replace `ServeDir` with `rust-embed`:

```toml
# Cargo.toml
[dependencies]
rust-embed = { version = "8", optional = true }

[features]
sqlite = ["sqlx/sqlite", "osmpbf"]
mobile = ["sqlite", "rust-embed"]
```

```rust
#[derive(rust_embed::Embed)]
#[folder = "app/"]
struct Assets;

// Axum handler that serves embedded files
async fn static_handler(uri: axum::http::Uri) -> impl axum::response::IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(axum::http::header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}
```

This makes the binary fully self-contained. No `app/` directory needed at runtime.

### Step 2: Extract `start_server()` from `ondevice` main

Move the server setup into a library function that can be called from native code:

```rust
// src/lib.rs (or a new src/mobile.rs)

pub struct ServerHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

pub struct ServerConfig {
    pub region_db_path: String,
    pub port: u16,
    pub mapbox_api_key: String,
    pub mapbox_base_url: Option<String>,
}

/// Start the on-device server. Returns a handle to shut it down.
/// Call from a background thread — this blocks until shutdown.
pub async fn start_server(config: ServerConfig) -> Result<ServerHandle, Box<dyn std::error::Error>> {
    // Same setup as ondevice main():
    // - Open SQLite pool
    // - Create MemoryCacheService
    // - Create MapboxClient
    // - Build RouteGenerator
    // - Build Router (core routes + embedded static fallback)
    // - Bind to 127.0.0.1:{port}
    // - Serve with graceful shutdown via oneshot channel
}
```

The `ondevice` binary becomes a thin wrapper:

```rust
#[tokio::main]
async fn main() {
    let config = parse_cli_args();
    let handle = easyroute::start_server(config).await.unwrap();
    // handle.shutdown_tx dropped on Ctrl-C
}
```

### Step 3: C FFI Entry Point

Expose `start_server` and `stop_server` as C-callable functions for the native shell:

```rust
// src/ffi.rs
use std::ffi::CStr;
use std::os::raw::c_char;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static HANDLE: Mutex<Option<ServerHandle>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn easyroute_start(
    region_path: *const c_char,
    port: u16,
    mapbox_key: *const c_char,
    proxy_url: *const c_char, // nullable
) -> i32 {
    let region_path = unsafe { CStr::from_ptr(region_path) }.to_str().unwrap();
    let mapbox_key = unsafe { CStr::from_ptr(mapbox_key) }.to_str().unwrap();
    let proxy_url = if proxy_url.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(proxy_url) }.to_str().unwrap().to_string())
    };

    let rt = RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().unwrap());
    let config = ServerConfig { /* ... */ };

    match rt.block_on(start_server(config)) {
        Ok(handle) => { *HANDLE.lock().unwrap() = Some(handle); 0 }
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn easyroute_stop() {
    if let Some(handle) = HANDLE.lock().unwrap().take() {
        let _ = handle.shutdown_tx.send(());
    }
}
```

Build as a static library:

```toml
[lib]
name = "easyroute"
crate-type = ["lib", "staticlib"]  # staticlib for iOS, cdylib for Android
```

### Step 4: iOS Shell (~80 lines of Swift)

Xcode project with a single `ContentView`:

```swift
import SwiftUI
import WebKit

@main
struct EasyRouteApp: App {
    init() {
        // Copy bundled region.db to app Documents if not present
        let dbPath = copyRegionIfNeeded()

        // Start Rust server on background thread
        DispatchQueue.global(qos: .userInitiated).async {
            let key = Bundle.main.object(forInfoDictionaryKey: "MAPBOX_API_KEY") as! String
            let proxy = Bundle.main.object(forInfoDictionaryKey: "MAPBOX_BASE_URL") as? String
            easyroute_start(dbPath, 3000, key, proxy)
        }
    }

    var body: some Scene {
        WindowGroup {
            WebView(url: URL(string: "http://127.0.0.1:3000")!)
                .ignoresSafeArea()
        }
    }
}

struct WebView: UIViewRepresentable {
    let url: URL
    func makeUIView(context: Context) -> WKWebView {
        let webView = WKWebView()
        webView.load(URLRequest(url: url))
        return webView
    }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
}
```

Build the Rust library for iOS:

```bash
rustup target add aarch64-apple-ios
cargo build --target aarch64-apple-ios --features mobile --lib --release
# Output: target/aarch64-apple-ios/release/libeasyroute.a
```

Link `libeasyroute.a` in the Xcode project and add a C bridging header declaring `easyroute_start` and `easyroute_stop`.

### Step 5: Android Shell (~60 lines of Kotlin)

```kotlin
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Copy bundled region.db to app files dir if not present
        val dbPath = copyRegionIfNeeded()
        val key = BuildConfig.MAPBOX_API_KEY
        val proxy = BuildConfig.MAPBOX_BASE_URL

        // Start Rust server on background thread
        thread { NativeLib.easyrouteStart(dbPath, 3000, key, proxy) }

        setContent {
            AndroidView(factory = { context ->
                WebView(context).apply {
                    settings.javaScriptEnabled = true
                    settings.domStorageEnabled = true
                    loadUrl("http://127.0.0.1:3000")
                }
            }, modifier = Modifier.fillMaxSize())
        }
    }

    override fun onDestroy() {
        NativeLib.easyrouteStop()
        super.onDestroy()
    }
}
```

Build the Rust library for Android:

```bash
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo ndk -t arm64-v8a -t armeabi-v7a build --features mobile --lib --release
# Output: target/aarch64-linux-android/release/libeasyroute.so
```

Place `.so` files in `app/src/main/jniLibs/{abi}/` and declare JNI bindings in a `NativeLib` class.

## Region Database Management

Region `.db` files (~100KB for Monaco, ~2MB for Ile-de-France) need to reach the device:

**Option 1: Bundled** — Ship one default region in the app binary. Simple, adds ~2MB to app size. Good for a single-region MVP.

**Option 2: Download on first launch** — App starts with no region, downloads from the proxy server on first run. Requires adding a `GET /v1/regions` endpoint to the proxy. Better for multi-region support.

**Option 3: Hybrid** — Bundle a small region (Monaco for testing), download others on demand. Store in the app's Documents directory.

The proxy already has the infrastructure for this — adding a static file download endpoint is trivial.

## Lifecycle Considerations

**App backgrounding (iOS):** iOS suspends background threads after ~30 seconds. The server shuts down, cache is lost (acceptable — it's in-memory). On foregrounding, restart the server. SQLite handles this gracefully — no open-transaction corruption risk since all queries are short-lived.

**App backgrounding (Android):** Similar. Use a foreground service if the server needs to persist (e.g., for route export while the app is backgrounded), otherwise let it stop.

**Memory:** The main memory consumers are the moka cache (bounded at 1000 entries) and the Tokio runtime. SQLite uses mmap (configured at 256MB in `ondevice`, should be reduced to ~32MB on mobile). Total footprint: ~30-50MB.

**Startup time:** SQLite pool creation + schema check + region metadata read takes <100ms. The WebView is the bottleneck (~500ms on modern devices). The server is ready before the WebView finishes loading.

## Mapbox Token Handling

The Mapbox GL JS library in the web UI needs an access token for map tiles (separate from the Directions API key held by the proxy). Two options:

1. **Public token** — Mapbox allows creating URL-restricted public tokens scoped to specific domains. Restrict to `localhost` and `127.0.0.1`. Embed in `main.js`. Low risk since it only grants tile access.

2. **Inject from native** — The native shell injects the token into the WebView via JavaScript before the page loads: `webView.evaluateJavaScript("window.MAPBOX_TOKEN = '...'")`. Keeps the token out of the JS source.

Option 2 is more secure. The `app/main.js` already checks `window.MAPBOX_TOKEN` before falling back to a prompt.

## What This Doesn't Give You

- **Native map feel** — WebView maps work but lack the smoothness of MapKit / Google Maps SDK. Pinch-to-zoom, rotation, and 3D tilt are functional but not pixel-perfect native.
- **System integration** — No "Open in Apple Maps" or share-to-Google-Maps without additional JavaScript-to-native bridging.
- **Offline map tiles** — Mapbox GL JS requires network for base map tiles. Offline maps need Mapbox's native SDKs or a tile cache.
- **Push notifications / widgets** — Not relevant for v1 but would require native code.

If any of these become blockers, the migration path is clear: replace the WebView with native UI screens that call the same `localhost` API. The Rust server doesn't change.
