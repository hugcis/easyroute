import CoreLocation
import SwiftUI

struct RouteControlsView: View {
    var routeState: RouteState
    var locationManager: LocationManager
    var apiClient: APIClient?
    var selectedDetent: PresentationDetent
    var onRoutesGenerated: () -> Void

    @State private var showCategoryPicker = false
    @State private var gpxShareURL: URL?
    @State private var showShareSheet = false

    private var isCollapsed: Bool { selectedDetent == .height(120) }

    var body: some View {
        ScrollView {
            if !isCollapsed {
                VStack(spacing: 16) {
                    remainingControls
                    expandedGenerateButton
                    errorBanner
                    routeCardsSection
                }
                .padding(.horizontal)
                .padding(.bottom, 20)
            }
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

    // MARK: - Pinned Header

    private var pinnedHeader: some View {
        VStack(spacing: 8) {
            Capsule()
                .fill(.quaternary)
                .frame(width: 36, height: 5)
                .padding(.top, 14)

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

            HStack(spacing: 12) {
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

                Spacer()

                Button {
                    Task { await generateRoutes() }
                } label: {
                    Group {
                        if routeState.isLoading {
                            ProgressView()
                                .tint(.white)
                                .controlSize(.small)
                        } else {
                            Label("Go", systemImage: "arrow.trianglehead.counterclockwise.rotate.90")
                        }
                    }
                    .fontWeight(.semibold)
                }
                .buttonStyle(.borderedProminent)
                .disabled(routeState.isLoading || routeState.mapCenter == nil || apiClient == nil)
            }
        }
        .padding(.horizontal)
        .padding(.bottom, 8)
    }

    // MARK: - Remaining Controls

    private var remainingControls: some View {
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

    // MARK: - Expanded Generate Button

    @ViewBuilder
    private var expandedGenerateButton: some View {
        Button {
            Task { await generateRoutes() }
        } label: {
            HStack {
                if routeState.isLoading {
                    ProgressView()
                        .tint(.white)
                } else {
                    Image(systemName: "arrow.trianglehead.counterclockwise.rotate.90")
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

        let categories = routeState.selectedCategories
        let preferences = categories.isEmpty
            ? nil
            : RoutePreferences(poiCategories: Array(categories))

        let request = LoopRouteRequest(
            startPoint: Coordinates(from: start),
            distanceKm: routeState.distanceKm,
            mode: routeState.mode,
            preferences: preferences
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
        let selected = routeState.selectedCategories
        if selected.isEmpty { return "All categories" }
        if selected.count > 2 { return "\(selected.count) categories" }
        return selected
            .compactMap { POICategories.allCategories[$0]?.label }
            .joined(separator: ", ")
    }
}
