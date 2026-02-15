use crate::mobile::{ServerConfig, ServerHandle};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::{Mutex, OnceLock};
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static HANDLE: Mutex<Option<ServerHandle>> = Mutex::new(None);

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("Failed to create Tokio runtime"))
}

/// # Safety
///
/// All pointer parameters must be valid, non-null, null-terminated C strings.
/// Returns the actual port (>0) on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn easyroute_start(
    region_path: *const c_char,
    port: u16,
    mapbox_key: *const c_char,
    proxy_url: *const c_char,
) -> i32 {
    let region_path = unsafe { CStr::from_ptr(region_path) }
        .to_string_lossy()
        .into_owned();
    let mapbox_key = unsafe { CStr::from_ptr(mapbox_key) }
        .to_string_lossy()
        .into_owned();
    let proxy_url = if proxy_url.is_null() {
        None
    } else {
        let s = unsafe { CStr::from_ptr(proxy_url) }
            .to_string_lossy()
            .into_owned();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };

    let config = ServerConfig {
        region_db_path: region_path,
        port,
        mapbox_api_key: mapbox_key,
        mapbox_base_url: proxy_url,
    };

    let rt = get_runtime();
    match rt.block_on(crate::mobile::start_server(config)) {
        Ok(handle) => {
            let port = handle.port as i32;
            if let Ok(mut guard) = HANDLE.lock() {
                *guard = Some(handle);
            }
            port
        }
        Err(e) => {
            eprintln!("easyroute_start failed: {e}");
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn easyroute_stop() {
    if let Ok(mut guard) = HANDLE.lock() {
        if let Some(handle) = guard.take() {
            let _ = handle.shutdown_tx.send(());
        }
    }
}
