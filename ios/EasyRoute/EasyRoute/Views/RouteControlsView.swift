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

    private var isCollapsed: Bool { selectedDetent == .height(192) }

    var body: some View {
        ScrollView {
            if !isCollapsed {
                VStack(spacing: 16) {
                    expandedGenerateButton
                    emptyPrompt
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
        VStack(spacing: 6) {
            Capsule()
                .fill(.quaternary)
                .frame(width: 36, height: 5)
                .padding(.top, 14)

            HStack {
                HStack(alignment: .firstTextBaseline, spacing: 4) {
                    Text(String(format: "%.1f", routeState.distanceKm))
                        .font(.system(size: 32, weight: .bold, design: .rounded))
                        .contentTransition(.numericText(value: routeState.distanceKm))
                        .animation(.snappy, value: routeState.distanceKm)
                    Text("km")
                        .font(.callout.weight(.medium))
                        .foregroundStyle(.secondary)
                }

                Spacer()

                goButton
                    .opacity(isCollapsed ? 1 : 0)
                    .scaleEffect(isCollapsed ? 1 : 0.5)
                    .animation(.snappy(duration: 0.25), value: isCollapsed)
            }

            Slider(value: Binding(
                get: { routeState.distanceKm },
                set: { routeState.distanceKm = $0 }
            ), in: 1...20, step: 0.5)

            HStack {
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
                .fixedSize()

                Spacer()

                categoryFilterButton
            }

            if isCollapsed && !routeState.routes.isEmpty {
                CompactRouteSwitcherView(routeState: routeState)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .padding(.horizontal)
        .padding(.bottom, 8)
        .animation(.snappy(duration: 0.25), value: isCollapsed)
        .animation(.snappy(duration: 0.25), value: routeState.routes.isEmpty)
    }

    // MARK: - Go Button

    private var goButton: some View {
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
                        .fontWeight(.semibold)
                }
            }
            .frame(height: 20)
        }
        .buttonStyle(.borderedProminent)
        .buttonBorderShape(.capsule)
        .disabled(routeState.isLoading || routeState.mapCenter == nil || apiClient == nil)
    }

    // MARK: - Category Filter Button

    private var categoryFilterButton: some View {
        Button {
            showCategoryPicker = true
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "line.3.horizontal.decrease.circle")
                Text(categoryLabel)
            }
            .font(.subheadline)
            .lineLimit(1)
        }
        .buttonStyle(.bordered)
        .buttonBorderShape(.capsule)
        .sheet(isPresented: $showCategoryPicker) {
            CategoryPickerView(selectedCategories: Binding(
                get: { routeState.selectedCategories },
                set: { routeState.selectedCategories = $0 }
            ))
            .presentationDetents([.medium, .large])
        }
    }

    // MARK: - Expanded Generate Button

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
        .buttonBorderShape(.capsule)
        .disabled(routeState.isLoading || routeState.mapCenter == nil || apiClient == nil)
    }

    // MARK: - Empty Prompt

    @ViewBuilder
    private var emptyPrompt: some View {
        if routeState.routes.isEmpty && routeState.error == nil && !routeState.isLoading {
            VStack(spacing: 8) {
                Image(systemName: routeState.mapCenter == nil
                    ? "mappin.and.ellipse"
                    : "point.topleft.down.to.point.bottomright.curvepath")
                    .font(.title2)
                    .foregroundStyle(.tertiary)
                Text(routeState.mapCenter == nil
                    ? "Move the map to set your start point"
                    : "Tap Go to discover routes")
                    .font(.subheadline)
                    .foregroundStyle(.tertiary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 24)
        }
    }

    // MARK: - Error Banner

    @ViewBuilder
    private var errorBanner: some View {
        if let error = routeState.error {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.orange)
                Text(error)
                    .font(.subheadline)
                Spacer()
                Button {
                    withAnimation { routeState.error = nil }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.tertiary)
                }
            }
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(.orange.opacity(0.08))
            )
        }
    }

    // MARK: - Route Cards

    @ViewBuilder
    private var routeCardsSection: some View {
        if !routeState.routes.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                Text("\(routeState.routes.count) routes")
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
                                withAnimation(.snappy) {
                                    routeState.selectedRouteIndex = index
                                    onRoutesGenerated()
                                }
                            }
                            .transition(.offset(y: 20).combined(with: .opacity))
                            .animation(.snappy.delay(0.05 * Double(index)), value: isCollapsed)
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
