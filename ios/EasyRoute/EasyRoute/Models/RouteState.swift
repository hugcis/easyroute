import CoreLocation
import Observation

@Observable
final class RouteState {
    // Inputs
    var mapCenter: CLLocationCoordinate2D?
    var distanceKm: Double = 5.0
    var mode: TransportMode = .walk
    var selectedCategories: Set<String> = []

    // Outputs
    var routes: [Route] = []
    var selectedRouteIndex: Int = 0
    var isLoading: Bool = false
    var error: String?

    var selectedRoute: Route? {
        guard selectedRouteIndex < routes.count else { return nil }
        return routes[selectedRouteIndex]
    }

    func clearRoutes() {
        routes = []
        selectedRouteIndex = 0
        error = nil
    }
}
