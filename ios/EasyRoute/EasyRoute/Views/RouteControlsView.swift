import CoreLocation
import SwiftUI

struct RouteControlsView: View {
    var routeState: RouteState
    var locationManager: LocationManager
    var apiClient: APIClient?
    var onRoutesGenerated: () -> Void

    @State private var showCategoryPicker = false
    @State private var gpxShareURL: URL?
    @State private var showShareSheet = false

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                remainingControls
                generateButton
                errorBanner
                routeCardsSection
            }
            .padding(.horizontal)
            .padding(.bottom, 20)
        }
        .safeAreaInset(edge: .top) {
            pinnedHeader
        }
        .sheet(isPresented: $showShareSheet) {
            if let url = gpxShareURL {
                ShareSheet(items: [url])
            }
        }
    }

    // MARK: - Pinned Header (visible at collapsed detent)

    private var pinnedHeader: some View {
        VStack(spacing: 8) {
            Capsule()
                .fill(.quaternary)
                .frame(width: 36, height: 5)
                .padding(.top, 8)

            // Distance slider with inline coordinates
            VStack(spacing: 4) {
                HStack {
                    Text("Distance")
                        .font(.subheadline.weight(.medium))
                    Spacer()
                    if let center = routeState.mapCenter {
                        Text(String(format: "%.4f, %.4f", center.latitude, center.longitude))
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.tertiary)
                    }
                    Text(String(format: "%.1f km", routeState.distanceKm))
                        .font(.subheadline.monospacedDigit().weight(.medium))
                        .foregroundStyle(.secondary)
                }
                Slider(value: Binding(
                    get: { routeState.distanceKm },
                    set: { routeState.distanceKm = $0 }
                ), in: 1...20, step: 0.5)
            }
            .padding(.horizontal)
            .padding(.bottom, 8)
        }
        .background(.regularMaterial)
    }

    // MARK: - Remaining Controls (in scroll area)

    @ViewBuilder
    private var remainingControls: some View {
        VStack(spacing: 14) {
            // Mode toggle
            HStack {
                Text("Mode")
                    .font(.subheadline.weight(.medium))
                Spacer()
                Picker("Mode", selection: Binding(
                    get: { routeState.mode },
                    set: { routeState.mode = $0 }
                )) {
                    Label("Walk", systemImage: "figure.walk")
                        .tag(TransportMode.walk)
                    Label("Bike", systemImage: "bicycle")
                        .tag(TransportMode.bike)
                }
                .pickerStyle(.segmented)
                .frame(width: 160)
            }

            // Category button
            HStack {
                Button {
                    showCategoryPicker = true
                } label: {
                    HStack {
                        Image(systemName: "line.3.horizontal.decrease.circle")
                        Text(categoryLabel)
                    }
                    .font(.subheadline)
                }
                .buttonStyle(.bordered)
                .sheet(isPresented: $showCategoryPicker) {
                    CategoryPickerView(selectedCategories: Binding(
                        get: { routeState.selectedCategories },
                        set: { routeState.selectedCategories = $0 }
                    ))
                    .presentationDetents([.medium, .large])
                }

                Spacer()
            }
        }
    }

    // MARK: - Generate Button

    @ViewBuilder
    private var generateButton: some View {
        Button {
            Task { await generateRoutes() }
        } label: {
            HStack {
                if routeState.isLoading {
                    ProgressView()
                        .tint(.white)
                } else {
                    Image(systemName: "point.topright.arrow.triangle.backward.to.point.bottomleft.scurvepath")
                }
                Text(routeState.isLoading ? "Generating..." : "Generate Route")
                    .fontWeight(.semibold)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
        }
        .buttonStyle(.borderedProminent)
        .disabled(routeState.isLoading || routeState.mapCenter == nil || apiClient == nil)

        if routeState.mapCenter == nil {
            Text("Move the map to position the pin on your start point")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
    }

    // MARK: - Error Banner

    @ViewBuilder
    private var errorBanner: some View {
        if let error = routeState.error {
            HStack {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.yellow)
                Text(error)
                    .font(.caption)
                Spacer()
                Button {
                    routeState.error = nil
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
            }
            .padding(10)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(.red.opacity(0.1))
            )
        }
    }

    // MARK: - Route Cards

    @ViewBuilder
    private var routeCardsSection: some View {
        if !routeState.routes.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("\(routeState.routes.count) routes generated")
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(.secondary)

                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 12) {
                        ForEach(Array(routeState.routes.enumerated()), id: \.element.id) { index, route in
                            RouteCardView(
                                route: route,
                                index: index,
                                isSelected: index == routeState.selectedRouteIndex,
                                onExportGPX: { exportGPX(route: route) }
                            )
                            .onTapGesture {
                                withAnimation {
                                    routeState.selectedRouteIndex = index
                                    onRoutesGenerated()
                                }
                            }
                        }
                    }
                    .scrollTargetLayout()
                }
                .scrollTargetBehavior(.viewAligned)
            }
        }
    }

    // MARK: - Actions

    private func generateRoutes() async {
        guard let apiClient,
              let start = routeState.mapCenter
        else { return }

        routeState.isLoading = true
        routeState.error = nil

        var prefs: RoutePreferences?
        if !routeState.selectedCategories.isEmpty {
            prefs = RoutePreferences(poiCategories: Array(routeState.selectedCategories))
        }

        let request = LoopRouteRequest(
            startPoint: Coordinates(from: start),
            distanceKm: routeState.distanceKm,
            mode: routeState.mode,
            preferences: prefs
        )

        do {
            let routes = try await apiClient.generateRoutes(request: request)
            routeState.routes = routes
            routeState.selectedRouteIndex = 0
            onRoutesGenerated()
        } catch {
            routeState.error = error.localizedDescription
        }

        routeState.isLoading = false
    }

    private func exportGPX(route: Route) {
        if let url = GPXExporter.shareGPX(route: route) {
            gpxShareURL = url
            showShareSheet = true
        }
    }

    private var categoryLabel: String {
        if routeState.selectedCategories.isEmpty {
            return "All categories"
        }
        let count = routeState.selectedCategories.count
        if count <= 2 {
            return routeState.selectedCategories
                .compactMap { POICategories.allCategories[$0]?.label }
                .joined(separator: ", ")
        }
        return "\(count) categories"
    }
}
