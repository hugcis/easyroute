import SwiftUI

@main
struct EasyRouteApp: App {
    @StateObject private var server = ServerBridge()
    @State private var regionManager = RegionManager()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(server)
                .environment(regionManager)
                .onAppear { startServer() }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .active {
                        server.ensureRunning()
                    }
                }
        }
    }

    private func startServer() {
        // Copy bundled region DB to Documents on first launch (legacy path)
        let fm = FileManager.default
        let docs = fm.urls(for: .documentDirectory, in: .userDomainMask).first!
        let dest = docs.appendingPathComponent("region.db")

        if !fm.fileExists(atPath: dest.path),
           let bundled = Bundle.main.path(forResource: "region", ofType: "db") {
            try? fm.copyItem(atPath: bundled, toPath: dest.path)
        }

        let mapboxKey = Bundle.main.infoDictionary?["MAPBOX_API_KEY"] as? String ?? ""
        let proxyUrl = Bundle.main.infoDictionary?["MAPBOX_BASE_URL"] as? String

        server.start(
            regionPath: regionManager.activeRegionPath,
            mapboxKey: mapboxKey,
            proxyUrl: proxyUrl
        )
    }
}
