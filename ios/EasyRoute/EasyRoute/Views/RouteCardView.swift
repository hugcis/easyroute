import SwiftUI

struct RouteCardView: View {
    let route: Route
    let index: Int
    let isSelected: Bool
    let onExportGPX: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Route \(index + 1)")
                    .font(.headline)
                Spacer()
                Text(String(format: "%.1f/10", route.score))
                    .font(.subheadline.bold())
                    .foregroundStyle(scoreColor)
            }

            HStack(spacing: 12) {
                Label(String(format: "%.1f km", route.distanceKm), systemImage: "arrow.triangle.swap")
                    .font(.subheadline)
                Label("\(route.estimatedDurationMinutes) min", systemImage: "clock")
                    .font(.subheadline)
                if let gain = route.elevationGainM, gain > 0 {
                    Label(String(format: "%.0f m", gain), systemImage: "arrow.up.right")
                        .font(.subheadline)
                }
            }
            .foregroundStyle(.secondary)

            if let m = route.metrics {
                metricsRow(m)
            }

            categoryChips

            HStack(spacing: 12) {
                Label("\(route.pois.count) waypoints", systemImage: "mappin.and.ellipse")
                    .font(.caption)
                if !route.snappedPois.isEmpty {
                    Label("\(route.snappedPois.count) nearby", systemImage: "mappin")
                        .font(.caption)
                }
                Spacer()
                Button(action: onExportGPX) {
                    Label("GPX", systemImage: "square.and.arrow.up")
                        .font(.caption.bold())
                }
                .buttonStyle(.bordered)
                .controlSize(.mini)
            }
            .foregroundStyle(.secondary)
        }
        .padding()
        .frame(width: 260)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(.regularMaterial)
                .overlay(
                    RoundedRectangle(cornerRadius: 12)
                        .stroke(isSelected ? Color.accentColor : .clear, lineWidth: 2)
                )
        )
    }

    // MARK: - Metrics row

    private func metricsRow(_ m: RouteMetrics) -> some View {
        HStack(spacing: 10) {
            if let v = m.circularity {
                metricGauge(label: "Loop", value: v)
            }
            if let v = m.pathOverlapPct {
                metricGauge(label: "Unique", value: max(0, 1.0 - v))
            }
            if let v = m.poiDensityPerKm {
                metricGauge(label: "Density", value: min(1.0, v / 2.0))
            }
        }
    }

    private func metricGauge(label: String, value: Float) -> some View {
        let clamped = CGFloat(min(1, max(0, value)))
        return VStack(spacing: 2) {
            Text(label)
                .font(.system(size: 9))
                .foregroundStyle(.secondary)
            RoundedRectangle(cornerRadius: 2)
                .fill(Color.secondary.opacity(0.2))
                .frame(width: 20, height: 4)
                .overlay(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 2)
                        .fill(gaugeColor(value))
                        .frame(width: 20 * clamped, height: 4)
                }
        }
    }

    private func gaugeColor(_ value: Float) -> Color {
        switch value {
        case 0.7...: .green
        case 0.4...: .orange
        default: .red
        }
    }

    // MARK: - Category chips

    private var categoryChips: some View {
        let allCategories = route.pois.map(\.category) + route.snappedPois.map(\.category)
        let uniqueCategories = Set(allCategories).sorted()
        let maxShown = 5
        let shown = uniqueCategories.prefix(maxShown)
        let overflow = uniqueCategories.count - maxShown

        return ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                ForEach(shown, id: \.self) { cat in
                    let info = POICategories.info(for: cat)
                    HStack(spacing: 3) {
                        Image(systemName: info.symbol)
                            .font(.system(size: 8))
                        Text(info.label)
                            .font(.system(size: 9))
                    }
                    .padding(.horizontal, 6)
                    .padding(.vertical, 3)
                    .background(Capsule().fill(info.color.opacity(0.15)))
                    .foregroundStyle(info.color)
                }
                if overflow > 0 {
                    Text("+\(overflow)")
                        .font(.system(size: 9).bold())
                        .padding(.horizontal, 6)
                        .padding(.vertical, 3)
                        .background(Capsule().fill(.quaternary))
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var scoreColor: Color {
        switch route.score {
        case 7...: .green
        case 4...: .orange
        default: .red
        }
    }
}
