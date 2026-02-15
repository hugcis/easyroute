import Foundation

enum APIError: LocalizedError {
    case serverNotReady
    case httpError(statusCode: Int, body: String)
    case decodingError(Error)
    case networkError(Error)

    var errorDescription: String? {
        switch self {
        case .serverNotReady:
            return "Server is not running"
        case .httpError(let code, let body):
            return "Server error (\(code)): \(body)"
        case .decodingError(let error):
            return "Invalid response: \(error.localizedDescription)"
        case .networkError(let error):
            return "Network error: \(error.localizedDescription)"
        }
    }
}

struct APIClient {
    let baseURL: URL
    private let session: URLSession

    init(port: Int32) {
        self.baseURL = URL(string: "http://127.0.0.1:\(port)")!
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 30
        self.session = URLSession(configuration: config)
    }

    func generateRoutes(request: LoopRouteRequest) async throws -> [Route] {
        let url = baseURL.appendingPathComponent("api/v1/routes/loop")
        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = "POST"
        urlRequest.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let encoder = JSONEncoder()
        urlRequest.httpBody = try encoder.encode(request)

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: urlRequest)
        } catch {
            throw APIError.networkError(error)
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            throw APIError.networkError(URLError(.badServerResponse))
        }

        guard httpResponse.statusCode == 200 else {
            let body = String(data: data, encoding: .utf8) ?? "Unknown error"
            throw APIError.httpError(statusCode: httpResponse.statusCode, body: body)
        }

        do {
            let decoder = JSONDecoder()
            let routeResponse = try decoder.decode(RouteResponse.self, from: data)
            return routeResponse.routes
        } catch {
            throw APIError.decodingError(error)
        }
    }

    func healthCheck() async -> Bool {
        let url = baseURL.appendingPathComponent("api/v1/debug/health")
        guard let (_, response) = try? await session.data(from: url),
              let http = response as? HTTPURLResponse else {
            return false
        }
        return http.statusCode == 200
    }
}
