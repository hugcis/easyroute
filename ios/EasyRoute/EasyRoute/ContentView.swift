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

    @ViewBuilder
    private var mainContent: some View {
        RouteMapView(
            routeState: routeState,
            cameraPosition: $cameraPosition
        )
        .sheet(isPresented: .constant(true)) {
            RouteControlsView(
                routeState: routeState,
                locationManager: locationManager,
                apiClient: apiClient,
                onRoutesGenerated: fitCameraToRoute
            )
            .presentationDetents(
                [.height(120), .medium, .large],
                selection: $selectedDetent
            )
            .presentationBackgroundInteraction(.enabled(upThrough: .medium))
            .interactiveDismissDisabled()
        }
    }

    // MARK: - Camera fitting

    private func fitCameraToRoute() {
        guard let route = routeState.selectedRoute else { return }
        withAnimation(.easeInOut(duration: 0.5)) {
            cameraPosition = RouteMapView.cameraFitting(route: route)
        }
        selectedDetent = .height(120)
    }
}
