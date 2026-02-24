import Foundation

// MARK: - Remote catalog (from proxy API)

struct RegionCatalogResponse: Decodable {
    let regions: [RemoteRegion]
}

struct RemoteRegion: Decodable, Identifiable {
    let id: String
    let name: String
    let sizeBytes: UInt64
    let poiCount: UInt64
    let buildDate: String

    enum CodingKeys: String, CodingKey {
        case id, name
        case sizeBytes = "size_bytes"
        case poiCount = "poi_count"
        case buildDate = "build_date"
    }

    var formattedSize: String {
        ByteCountFormatter.string(fromByteCount: Int64(sizeBytes), countStyle: .file)
    }

    var formattedPoiCount: String {
        poiCount.formatted()
    }
}

// MARK: - Downloaded region (persisted on device)

struct DownloadedRegion: Codable, Identifiable {
    let id: String
    let name: String
    let sizeBytes: UInt64
    let poiCount: UInt64
    let downloadDate: Date

    var filename: String { "\(id).db" }

    var formattedSize: String {
        ByteCountFormatter.string(fromByteCount: Int64(sizeBytes), countStyle: .file)
    }

    var formattedPoiCount: String {
        poiCount.formatted()
    }
}
