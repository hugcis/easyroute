import Combine
import Foundation

class ServerBridge: ObservableObject {
    @Published var port: Int32 = 0
    @Published var isRunning = false

    func start(regionPath: String, mapboxKey: String, proxyUrl: String?) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let result = easyroute_start(
                regionPath,
                3000,
                mapboxKey,
                proxyUrl
            )

            DispatchQueue.main.async {
                if result > 0 {
                    self?.port = result
                    self?.isRunning = true
                }
            }
        }
    }

    func stop() {
        easyroute_stop()
        isRunning = false
        port = 0
    }
}
