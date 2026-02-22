import MapKit
import SwiftUI

struct RouteMapView: View {
    var routeState: RouteState
    @Binding var cameraPosition: MapCameraPosition

    var body: some View {
        ZStack {
            mapContent
            centerPin
        }
        .sheet(isPresented: Binding(
            get: { routeState.selectedPoi != nil },
            set: { if !$0 { routeState.selectedPoi = nil } }
        )) {
            if let poi = routeState.selectedPoi {
                POIDetailView(poi: poi) {
                    routeState.selectedPoi = nil
                }
                .presentationDetents([.height(200), .medium])
                .presentationDragIndicator(.visible)
                .presentationBackgroundInteraction(.enabled(upThrough: .height(200)))
            }
        }
    }

    private var mapContent: some View {
        Map(position: $cameraPosition) {
            UserAnnotation()

            if let route = routeState.selectedRoute {
                // Route polyline
                MapPolyline(coordinates: route.path.map(\.clLocationCoordinate))
                    .stroke(.blue, lineWidth: 4)

                // Start/End marker
                if let start = route.path.first {
                    Annotation("Start/End", coordinate: start.clLocationCoordinate) {
                        StartMarkerView()
                    }
                }

                // Waypoint POIs
                ForEach(route.pois) { poi in
                    Annotation(poi.name, coordinate: poi.coordinates.clLocationCoordinate) {
                        WaypointMarkerView(order: poi.orderInRoute, category: poi.category)
                            .contentShape(Circle().size(width: 44, height: 44))
                            .onTapGesture {
                                routeState.selectedPoi = .waypoint(poi)
                            }
                    }
                }

                // Snapped POIs
                ForEach(route.snappedPois) { poi in
                    Annotation(poi.name, coordinate: poi.coordinates.clLocationCoordinate) {
                        SnappedMarkerView(category: poi.category)
                            .contentShape(Circle().size(width: 44, height: 44))
                            .onTapGesture {
                                routeState.selectedPoi = .snapped(poi)
                            }
                    }
                }
            }
        }
        .mapControls {
            MapUserLocationButton()
            MapCompass()
            MapScaleView()
        }
        .onMapCameraChange(frequency: .onEnd) { context in
            routeState.mapCenter = context.region.center
            routeState.selectedPoi = nil
        }
    }

    // Fixed center pin overlay
    private var centerPin: some View {
        VStack(spacing: 0) {
            Image(systemName: "mappin")
                .font(.title)
                .foregroundStyle(.red)
            // Small shadow dot at the pin tip
            Circle()
                .fill(.black.opacity(0.2))
                .frame(width: 6, height: 6)
                .offset(y: -2)
        }
        .allowsHitTesting(false)
    }

    // Compute a MapCameraPosition that fits all route path coordinates
    static func cameraFitting(route: Route) -> MapCameraPosition {
        let coords = route.path.map(\.clLocationCoordinate)
        guard !coords.isEmpty else { return .automatic }

        var minLat = coords[0].latitude
        var maxLat = coords[0].latitude
        var minLng = coords[0].longitude
        var maxLng = coords[0].longitude

        for coord in coords {
            minLat = min(minLat, coord.latitude)
            maxLat = max(maxLat, coord.latitude)
            minLng = min(minLng, coord.longitude)
            maxLng = max(maxLng, coord.longitude)
        }

        let latPad = (maxLat - minLat) * 0.15
        let lngPad = (maxLng - minLng) * 0.15

        let center = CLLocationCoordinate2D(
            latitude: (minLat + maxLat) / 2,
            longitude: (minLng + maxLng) / 2
        )
        let span = MKCoordinateSpan(
            latitudeDelta: (maxLat - minLat) + latPad * 2,
            longitudeDelta: (maxLng - minLng) + lngPad * 2
        )

        return .region(MKCoordinateRegion(center: center, span: span))
    }
}
