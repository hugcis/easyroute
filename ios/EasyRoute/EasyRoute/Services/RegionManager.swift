import Foundation
import Observation

@Observable
final class RegionManager {
    // Remote catalog
    var catalog: [RemoteRegion] = []
    var isFetchingCatalog = false
    var catalogError: String?

    // Local state
    private(set) var downloadedRegions: [DownloadedRegion] = []
    var activeRegionId: String? {
        didSet { UserDefaults.standard.set(activeRegionId, forKey: "activeRegionId") }
    }

    // Downloads in flight: regionId -> progress (0.0–1.0)
    var downloads: [String: Double] = [:]

    // Proxy config
    private let proxyBaseURL: String?
    private let proxyAPIKey: String?

    // Paths
    private let regionsDir: URL
    private let metadataURL: URL
    private let bundledRegionPath: String?

    init() {
        let fm = FileManager.default
        let docs = fm.urls(for: .documentDirectory, in: .userDomainMask).first!
        regionsDir = docs.appendingPathComponent("regions")
        metadataURL = docs.appendingPathComponent("regions-metadata.json")

        // Ensure regions directory exists
        try? fm.createDirectory(at: regionsDir, withIntermediateDirectories: true)

        // Clean up stale .tmp files
        if let files = try? fm.contentsOfDirectory(at: regionsDir, includingPropertiesForKeys: nil) {
            for file in files where file.pathExtension == "tmp" {
                try? fm.removeItem(at: file)
            }
        }

        // Read proxy config from Info.plist
        let info = Bundle.main.infoDictionary
        let baseURL = info?["PROXY_BASE_URL"] as? String
        proxyBaseURL = (baseURL?.isEmpty == false) ? baseURL : nil
        let apiKey = info?["PROXY_API_KEY"] as? String
        proxyAPIKey = (apiKey?.isEmpty == false) ? apiKey : nil

        // Bundled region fallback
        bundledRegionPath = Bundle.main.path(forResource: "region", ofType: "db")

        // Load persisted state
        activeRegionId = UserDefaults.standard.string(forKey: "activeRegionId")
        loadMetadata()
    }

    // MARK: - Active region

    var activeRegionPath: String {
        if let id = activeRegionId {
            let path = regionsDir.appendingPathComponent("\(id).db").path
            if FileManager.default.fileExists(atPath: path) {
                return path
            }
        }
        // Fall back to legacy Documents/region.db, then bundled
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        let legacy = docs.appendingPathComponent("region.db").path
        if FileManager.default.fileExists(atPath: legacy) {
            return legacy
        }
        return bundledRegionPath ?? ""
    }

    var activeRegionName: String {
        if let id = activeRegionId,
           let region = downloadedRegions.first(where: { $0.id == id }) {
            return region.name
        }
        return "Default"
    }

    func setActiveRegion(id: String) {
        activeRegionId = id
    }

    // MARK: - Catalog

    func fetchCatalog() async {
        guard let baseURL = proxyBaseURL, let apiKey = proxyAPIKey else {
            catalogError = "Proxy not configured"
            return
        }

        isFetchingCatalog = true
        catalogError = nil

        do {
            var request = URLRequest(url: URL(string: "\(baseURL)/v1/regions")!)
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")

            let (data, response) = try await URLSession.shared.data(for: request)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                catalogError = "Failed to fetch catalog"
                isFetchingCatalog = false
                return
            }

            let decoded = try JSONDecoder().decode(RegionCatalogResponse.self, from: data)
            catalog = decoded.regions
        } catch {
            catalogError = error.localizedDescription
        }

        isFetchingCatalog = false
    }

    // MARK: - Download

    func downloadRegion(_ region: RemoteRegion) async {
        guard let baseURL = proxyBaseURL, let apiKey = proxyAPIKey else { return }

        let tmpURL = regionsDir.appendingPathComponent("\(region.id).db.tmp")
        let destURL = regionsDir.appendingPathComponent("\(region.id).db")

        downloads[region.id] = 0.0

        do {
            var request = URLRequest(url: URL(string: "\(baseURL)/v1/regions/\(region.id)/download")!)
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")

            let (asyncBytes, response) = try await URLSession.shared.bytes(for: request)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                downloads.removeValue(forKey: region.id)
                return
            }

            let totalSize = http.expectedContentLength
            let handle = try FileHandle(forWritingTo: {
                FileManager.default.createFile(atPath: tmpURL.path, contents: nil)
                return tmpURL
            }())

            var written: Int64 = 0
            let bufferSize = 256 * 1024
            var buffer = Data(capacity: bufferSize)

            for try await byte in asyncBytes {
                buffer.append(byte)
                if buffer.count >= bufferSize {
                    handle.write(buffer)
                    written += Int64(buffer.count)
                    buffer.removeAll(keepingCapacity: true)
                    if totalSize > 0 {
                        downloads[region.id] = Double(written) / Double(totalSize)
                    }
                }
            }

            if !buffer.isEmpty {
                handle.write(buffer)
            }
            handle.closeFile()

            // Move temp to final
            let fm = FileManager.default
            if fm.fileExists(atPath: destURL.path) {
                try fm.removeItem(at: destURL)
            }
            try fm.moveItem(at: tmpURL, to: destURL)

            // Record in metadata
            let downloaded = DownloadedRegion(
                id: region.id,
                name: region.name,
                sizeBytes: region.sizeBytes,
                poiCount: region.poiCount,
                downloadDate: Date()
            )
            downloadedRegions.removeAll { $0.id == region.id }
            downloadedRegions.append(downloaded)
            saveMetadata()
        } catch {
            try? FileManager.default.removeItem(at: tmpURL)
        }

        downloads.removeValue(forKey: region.id)
    }

    // MARK: - Delete

    func deleteRegion(id: String) {
        let fileURL = regionsDir.appendingPathComponent("\(id).db")
        try? FileManager.default.removeItem(at: fileURL)

        downloadedRegions.removeAll { $0.id == id }
        saveMetadata()

        if activeRegionId == id {
            activeRegionId = nil
        }
    }

    // MARK: - Persistence

    private func loadMetadata() {
        guard let data = try? Data(contentsOf: metadataURL),
              let decoded = try? JSONDecoder().decode([DownloadedRegion].self, from: data) else {
            return
        }
        // Only keep entries whose files still exist
        downloadedRegions = decoded.filter {
            FileManager.default.fileExists(atPath: regionsDir.appendingPathComponent($0.filename).path)
        }
    }

    private func saveMetadata() {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        guard let data = try? encoder.encode(downloadedRegions) else { return }
        try? data.write(to: metadataURL)
    }
}
