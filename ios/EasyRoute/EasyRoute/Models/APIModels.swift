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

struct Route: Decodable, Identifiable {
    let id: UUID
    let distanceKm: Double
    let estimatedDurationMinutes: Int
    let elevationGainM: Float?
    let path: [Coordinates]
    let pois: [RoutePoi]
    let snappedPois: [SnappedPoi]
    let score: Float

    enum CodingKeys: String, CodingKey {
        case id
        case distanceKm = "distance_km"
        case estimatedDurationMinutes = "estimated_duration_minutes"
        case elevationGainM = "elevation_gain_m"
        case path, pois
        case snappedPois = "snapped_pois"
        case score
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

    enum CodingKeys: String, CodingKey {
        case id, name, category, coordinates
        case popularityScore = "popularity_score"
        case description
        case orderInRoute = "order_in_route"
        case distanceFromStartKm = "distance_from_start_km"
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

    enum CodingKeys: String, CodingKey {
        case id, name, category, coordinates
        case popularityScore = "popularity_score"
        case description
        case distanceFromStartKm = "distance_from_start_km"
        case distanceFromPathM = "distance_from_path_m"
    }
}
