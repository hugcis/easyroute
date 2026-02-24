import Combine
import Foundation

class ServerBridge: ObservableObject {
    @Published var port: Int32 = 0
    @Published var isRunning = false

    // Stored for restart
    private var regionPath: String?
    private var mapboxKey: String?
    private var proxyUrl: String?

    private var isStarting = false

    func start(regionPath: String, mapboxKey: String, proxyUrl: String?) {
        self.regionPath = regionPath
        self.mapboxKey = mapboxKey
        self.proxyUrl = proxyUrl

        startServer()
    }

    /// Check server health and restart if it's dead (call on foreground return).
    func ensureRunning() {
        guard !isStarting else { return }

        let url = URL(string: "http://127.0.0.1:\(port)/api/v1/debug/health")!
        let request = URLRequest(url: url, timeoutInterval: 2)

        URLSession.shared.dataTask(with: request) { [weak self] _, response, error in
            guard let self else { return }

            let alive = (response as? HTTPURLResponse)?.statusCode == 200
            DispatchQueue.main.async {
                if alive {
                    self.isRunning = true
                } else {
                    self.isRunning = false
                    self.restart()
                }
            }
        }.resume()
    }

    func stop() {
        easyroute_stop()
        isRunning = false
        port = 0
    }

    func switchRegion(regionPath: String) {
        self.regionPath = regionPath
        restart()
    }

    // MARK: - Private

    private func restart() {
        stop()
        startServer()
    }

    private func startServer() {
        guard let regionPath, let mapboxKey, !isStarting else { return }
        isStarting = true

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let result = easyroute_start(
                regionPath,
                3000,
                mapboxKey,
                self?.proxyUrl
            )

            DispatchQueue.main.async {
                self?.isStarting = false
                if result > 0 {
                    self?.port = result
                    self?.isRunning = true
                }
            }
        }
    }
}
