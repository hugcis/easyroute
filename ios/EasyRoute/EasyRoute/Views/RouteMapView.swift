import MapKit
import SwiftUI

struct RouteMapView: View {
    var routeState: RouteState
    @Binding var cameraPosition: MapCameraPosition
    var selectedDetent: PresentationDetent

    @State private var currentRegion: MKCoordinateRegion?

    var body: some View {
        GeometryReader { geometry in
            let viewHeight = geometry.size.height
            let pinOffset = drawerHeight(for: selectedDetent, viewHeight: viewHeight) / 2

            ZStack {
                mapContent(viewHeight: viewHeight, pinOffset: pinOffset)
                centerPin
                    .offset(y: -pinOffset)
                    .animation(.easeInOut(duration: 0.25), value: selectedDetent)
            }
            .onChange(of: selectedDetent) { oldDetent, newDetent in
                guard let region = currentRegion, viewHeight > 0 else { return }
                let oldPinOffset = drawerHeight(for: oldDetent, viewHeight: viewHeight) / 2
                let newPinOffset = drawerHeight(for: newDetent, viewHeight: viewHeight) / 2
                let deltaLat = (newPinOffset - oldPinOffset) / viewHeight * region.span.latitudeDelta

                withAnimation(.easeInOut(duration: 0.25)) {
                    cameraPosition = .region(MKCoordinateRegion(
                        center: CLLocationCoordinate2D(
                            latitude: region.center.latitude - deltaLat,
                            longitude: region.center.longitude
                        ),
                        span: region.span
                    ))
                }
            }
        }
        .sheet(isPresented: Binding(
            get: { routeState.selectedPoi != nil },
            set: { if !$0 { routeState.selectedPoi = nil } }
        )) {
            if let poi = routeState.selectedPoi {
                POIDetailView(poi: poi) {
                    routeState.selectedPoi = nil
                }
                .presentationDetents([.height(200)])
                .presentationDragIndicator(.visible)
                .presentationBackgroundInteraction(.enabled(upThrough: .height(200)))
            }
        }
    }

    private func mapContent(viewHeight: CGFloat, pinOffset: CGFloat) -> some View {
        Map(position: $cameraPosition) {
            UserAnnotation()

            if let route = routeState.selectedRoute {
                MapPolyline(coordinates: route.path.map(\.clLocationCoordinate))
                    .stroke(.blue, lineWidth: 4)

                if let start = route.path.first {
                    Annotation("Start/End", coordinate: start.clLocationCoordinate) {
                        StartMarkerView()
                    }
                }

                ForEach(route.pois) { poi in
                    Annotation(poi.name, coordinate: poi.coordinates.clLocationCoordinate) {
                        WaypointMarkerView(order: poi.orderInRoute, category: poi.category)
                            .contentShape(Circle().size(width: 44, height: 44))
                            .onTapGesture {
                                routeState.selectedPoi = .waypoint(poi)
                            }
                    }
                }

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
            currentRegion = context.region
            let latAdjustment = viewHeight > 0
                ? (pinOffset / viewHeight) * context.region.span.latitudeDelta
                : 0
            routeState.mapCenter = CLLocationCoordinate2D(
                latitude: context.region.center.latitude + latAdjustment,
                longitude: context.region.center.longitude
            )
            routeState.selectedPoi = nil
        }
    }

    private func drawerHeight(for detent: PresentationDetent, viewHeight: CGFloat) -> CGFloat {
        switch detent {
        case .height(120): 120
        case .medium: viewHeight * 0.47
        case .large: viewHeight * 0.88
        default: 0
        }
    }

    private var centerPin: some View {
        VStack(spacing: 0) {
            Image(systemName: "mappin")
                .font(.title)
                .foregroundStyle(.red)
            Circle()
                .fill(.black.opacity(0.2))
                .frame(width: 6, height: 6)
                .offset(y: -2)
        }
        .allowsHitTesting(false)
    }

    static func cameraFitting(route: Route) -> MapCameraPosition {
        let coords = route.path.map(\.clLocationCoordinate)
        guard let first = coords.first else { return .automatic }

        let (minLat, maxLat, minLng, maxLng) = coords.reduce(
            (first.latitude, first.latitude, first.longitude, first.longitude)
        ) { bounds, coord in
            (min(bounds.0, coord.latitude), max(bounds.1, coord.latitude),
             min(bounds.2, coord.longitude), max(bounds.3, coord.longitude))
        }

        let padding = 0.3 // 15% on each side
        let latSpan = (maxLat - minLat) * (1 + padding)
        let lngSpan = (maxLng - minLng) * (1 + padding)

        return .region(MKCoordinateRegion(
            center: CLLocationCoordinate2D(
                latitude: (minLat + maxLat) / 2,
                longitude: (minLng + maxLng) / 2
            ),
            span: MKCoordinateSpan(latitudeDelta: latSpan, longitudeDelta: lngSpan)
        ))
    }
}
