import MapKit
import SwiftUI

struct ContentView: View {
    @EnvironmentObject var server: ServerBridge

    @State private var routeState = RouteState()
    @State private var locationManager = LocationManager()
    @State private var apiClient: APIClient?

    @State private var cameraPosition: MapCameraPosition = .userLocation(fallback: .automatic)
    @State private var selectedDetent: PresentationDetent = .medium

    var body: some View {
        Group {
            if server.isRunning {
                mainContent
                    .onAppear {
                        if apiClient == nil {
                            apiClient = APIClient(port: server.port)
                        }
                        locationManager.requestPermission()
                    }
            } else {
                VStack(spacing: 16) {
                    ProgressView()
                        .scaleEffect(1.5)
                    Text("Starting server...")
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var mainContent: some View {
        RouteMapView(
            routeState: routeState,
            cameraPosition: $cameraPosition,
            selectedDetent: selectedDetent
        )
        .sheet(isPresented: .constant(true)) {
            RouteControlsView(
                routeState: routeState,
                locationManager: locationManager,
                apiClient: apiClient,
                selectedDetent: selectedDetent,
                onRoutesGenerated: fitCameraToRoute
            )
            .presentationDetents(
                [.height(192), .medium, .large],
                selection: $selectedDetent
            )
            .presentationBackgroundInteraction(.enabled(upThrough: .medium))
            .presentationDragIndicator(.hidden)
            .interactiveDismissDisabled()
        }
        .onChange(of: routeState.selectedRouteIndex) { _, _ in
            guard let route = routeState.selectedRoute,
                  selectedDetent == .height(192)
            else { return }
            withAnimation(.easeInOut(duration: 0.5)) {
                cameraPosition = RouteMapView.cameraFitting(route: route)
            }
        }
    }

    // MARK: - Camera fitting

    private func fitCameraToRoute() {
        guard let route = routeState.selectedRoute else { return }
        withAnimation(.easeInOut(duration: 0.5)) {
            cameraPosition = RouteMapView.cameraFitting(route: route)
        }
        selectedDetent = .height(192)
    }
}
