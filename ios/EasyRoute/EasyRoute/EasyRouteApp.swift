import SwiftUI

@main
struct EasyRouteApp: App {
    @StateObject private var server = ServerBridge()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(server)
                .onAppear { startServer() }
        }
    }

    private func startServer() {
        // Copy bundled region DB to Documents on first launch
        let fm = FileManager.default
        let docs = fm.urls(for: .documentDirectory, in: .userDomainMask).first!
        let dest = docs.appendingPathComponent("region.db")

        if !fm.fileExists(atPath: dest.path),
           let bundled = Bundle.main.path(forResource: "region", ofType: "db") {
            try? fm.copyItem(atPath: bundled, toPath: dest.path)
        }

        let mapboxKey = Bundle.main.infoDictionary?["MAPBOX_API_KEY"] as? String ?? ""
        let proxyUrl = Bundle.main.infoDictionary?["MAPBOX_BASE_URL"] as? String

        server.start(regionPath: dest.path, mapboxKey: mapboxKey, proxyUrl: proxyUrl)
    }
}
