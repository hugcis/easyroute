import Foundation
import CoreLocation

// MARK: - Request types

struct LoopRouteRequest: Encodable {
    let startPoint: Coordinates
    let distanceKm: Double
    let mode: TransportMode
    var preferences: RoutePreferences?

    enum CodingKeys: String, CodingKey {
        case startPoint = "start_point"
        case distanceKm = "distance_km"
        case mode
        case preferences
    }
}

struct RoutePreferences: Encodable {
    var poiCategories: [String]?
    var hiddenGems: Bool?

    enum CodingKeys: String, CodingKey {
        case poiCategories = "poi_categories"
        case hiddenGems = "hidden_gems"
    }
}

enum TransportMode: String, Codable, CaseIterable {
    case walk
    case bike
}

// MARK: - Response types

struct RouteResponse: Decodable {
    let routes: [Route]
}

struct RouteMetrics: Decodable {
    let circularity: Float?
    let convexity: Float?
    let pathOverlapPct: Float?
    let poiDensityPerKm: Float?
    let categoryEntropy: Float?
    let landmarkCoverage: Float?
    let poiDensityContext: String?

    enum CodingKeys: String, CodingKey {
        case circularity, convexity
        case pathOverlapPct = "path_overlap_pct"
        case poiDensityPerKm = "poi_density_per_km"
        case categoryEntropy = "category_entropy"
        case landmarkCoverage = "landmark_coverage"
        case poiDensityContext = "poi_density_context"
    }
}

struct Route: Decodable, Identifiable {
    let id: UUID
    let distanceKm: Double
    let estimatedDurationMinutes: Int
    let elevationGainM: Float?
    let path: [Coordinates]
    let pois: [RoutePoi]
    let snappedPois: [SnappedPoi]
    let score: Float
    let metrics: RouteMetrics?

    enum CodingKeys: String, CodingKey {
        case id
        case distanceKm = "distance_km"
        case estimatedDurationMinutes = "estimated_duration_minutes"
        case elevationGainM = "elevation_gain_m"
        case path, pois
        case snappedPois = "snapped_pois"
        case score, metrics
    }
}

struct Coordinates: Codable {
    let lat: Double
    let lng: Double

    var clLocationCoordinate: CLLocationCoordinate2D {
        CLLocationCoordinate2D(latitude: lat, longitude: lng)
    }

    init(lat: Double, lng: Double) {
        self.lat = lat
        self.lng = lng
    }

    init(from coordinate: CLLocationCoordinate2D) {
        self.lat = coordinate.latitude
        self.lng = coordinate.longitude
    }
}

struct RoutePoi: Decodable, Identifiable {
    let id: UUID
    let name: String
    let category: String
    let coordinates: Coordinates
    let popularityScore: Float
    let description: String?
    let orderInRoute: Int
    let distanceFromStartKm: Double
    let estimatedVisitDurationMinutes: Int?

    enum CodingKeys: String, CodingKey {
        case id, name, category, coordinates
        case popularityScore = "popularity_score"
        case description
        case orderInRoute = "order_in_route"
        case distanceFromStartKm = "distance_from_start_km"
        case estimatedVisitDurationMinutes = "estimated_visit_duration_minutes"
    }
}

struct SnappedPoi: Decodable, Identifiable {
    let id: UUID
    let name: String
    let category: String
    let coordinates: Coordinates
    let popularityScore: Float
    let description: String?
    let distanceFromStartKm: Double
    let distanceFromPathM: Float
    let estimatedVisitDurationMinutes: Int?

    enum CodingKeys: String, CodingKey {
        case id, name, category, coordinates
        case popularityScore = "popularity_score"
        case description
        case distanceFromStartKm = "distance_from_start_km"
        case distanceFromPathM = "distance_from_path_m"
        case estimatedVisitDurationMinutes = "estimated_visit_duration_minutes"
    }
}

// MARK: - Selected POI (for detail sheet)

enum SelectedPoi: Identifiable, Equatable {
    case waypoint(RoutePoi)
    case snapped(SnappedPoi)

    var id: UUID {
        switch self {
        case .waypoint(let poi): poi.id
        case .snapped(let poi): poi.id
        }
    }

    var name: String {
        switch self {
        case .waypoint(let poi): poi.name
        case .snapped(let poi): poi.name
        }
    }

    var category: String {
        switch self {
        case .waypoint(let poi): poi.category
        case .snapped(let poi): poi.category
        }
    }

    var coordinates: Coordinates {
        switch self {
        case .waypoint(let poi): poi.coordinates
        case .snapped(let poi): poi.coordinates
        }
    }

    var popularityScore: Float {
        switch self {
        case .waypoint(let poi): poi.popularityScore
        case .snapped(let poi): poi.popularityScore
        }
    }

    var description: String? {
        switch self {
        case .waypoint(let poi): poi.description
        case .snapped(let poi): poi.description
        }
    }

    var distanceFromStartKm: Double {
        switch self {
        case .waypoint(let poi): poi.distanceFromStartKm
        case .snapped(let poi): poi.distanceFromStartKm
        }
    }

    var estimatedVisitDurationMinutes: Int? {
        switch self {
        case .waypoint(let poi): poi.estimatedVisitDurationMinutes
        case .snapped(let poi): poi.estimatedVisitDurationMinutes
        }
    }

    var orderInRoute: Int? {
        switch self {
        case .waypoint(let poi): poi.orderInRoute
        case .snapped: nil
        }
    }

    var distanceFromPathM: Float? {
        switch self {
        case .waypoint: nil
        case .snapped(let poi): poi.distanceFromPathM
        }
    }

    var isWaypoint: Bool {
        if case .waypoint = self { return true }
        return false
    }

    static func == (lhs: SelectedPoi, rhs: SelectedPoi) -> Bool {
        lhs.id == rhs.id
    }
}

extension RoutePoi: Equatable {
    static func == (lhs: RoutePoi, rhs: RoutePoi) -> Bool {
        lhs.id == rhs.id
    }
}

extension SnappedPoi: Equatable {
    static func == (lhs: SnappedPoi, rhs: SnappedPoi) -> Bool {
        lhs.id == rhs.id
    }
}
