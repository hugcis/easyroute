import SwiftUI

struct RouteCardView: View {
    let route: Route
    let index: Int
    let isSelected: Bool
    let onExportGPX: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Route \(index + 1)")
                    .font(.headline)
                Spacer()
                scoreBadge
            }

            HStack(spacing: 14) {
                statItem(icon: "arrow.triangle.swap", value: String(format: "%.1f km", route.distanceKm))
                statItem(icon: "clock", value: "\(route.estimatedDurationMinutes) min")
                if let gain = route.elevationGainM, gain > 0 {
                    statItem(icon: "arrow.up.right", value: String(format: "%.0fm", gain))
                }
            }
            .foregroundStyle(.secondary)

            if let m = route.metrics {
                metricsRow(m)
            }

            categoryChips

            HStack(spacing: 10) {
                Label("\(route.pois.count) stops", systemImage: "mappin.and.ellipse")
                if !route.snappedPois.isEmpty {
                    Label("\(route.snappedPois.count) nearby", systemImage: "mappin")
                }
                Spacer()
                Button(action: onExportGPX) {
                    Image(systemName: "square.and.arrow.up")
                        .font(.caption.bold())
                }
                .buttonStyle(.bordered)
                .buttonBorderShape(.circle)
                .controlSize(.mini)
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .padding()
        .frame(width: 270)
        .background {
            RoundedRectangle(cornerRadius: 14)
                .fill(.regularMaterial)
                .shadow(color: .black.opacity(isSelected ? 0.1 : 0.05), radius: isSelected ? 8 : 4, y: 2)
                .overlay {
                    RoundedRectangle(cornerRadius: 14)
                        .strokeBorder(isSelected ? Color.accentColor : .clear, lineWidth: 2.5)
                }
        }
        .scaleEffect(isSelected ? 1.0 : 0.97)
        .animation(.snappy(duration: 0.25), value: isSelected)
    }

    // MARK: - Score Badge

    private var scoreBadge: some View {
        HStack(alignment: .firstTextBaseline, spacing: 2) {
            Text(String(format: "%.1f", route.score))
                .font(.subheadline.bold().monospacedDigit())
            Text("/10")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(route.scoreColor.opacity(0.12), in: Capsule())
        .foregroundStyle(route.scoreColor)
    }

    // MARK: - Stats

    private func statItem(icon: String, value: String) -> some View {
        HStack(spacing: 3) {
            Image(systemName: icon)
                .font(.caption2)
            Text(value)
                .font(.caption.monospacedDigit())
        }
    }

    // MARK: - Metrics

    private func metricsRow(_ m: RouteMetrics) -> some View {
        HStack(spacing: 14) {
            if let v = m.circularity {
                miniRing(value: v, label: "Loop")
            }
            if let v = m.pathOverlapPct {
                miniRing(value: max(0, 1.0 - v), label: "Unique")
            }
            if let v = m.poiDensityPerKm {
                miniRing(value: min(1.0, v / 2.0), label: "Density")
            }
            Spacer()
        }
    }

    private func miniRing(value: Float, label: String) -> some View {
        let clamped = CGFloat(min(1, max(0.01, value)))
        let color = gaugeColor(value)
        return VStack(spacing: 3) {
            ZStack {
                Circle()
                    .stroke(color.opacity(0.15), lineWidth: 3)
                Circle()
                    .trim(from: 0, to: clamped)
                    .stroke(color, style: StrokeStyle(lineWidth: 3, lineCap: .round))
                    .rotationEffect(.degrees(-90))
            }
            .frame(width: 26, height: 26)

            Text(label)
                .font(.system(size: 9))
                .foregroundStyle(.secondary)
        }
    }

    private func gaugeColor(_ value: Float) -> Color {
        switch value {
        case 0.7...: .green
        case 0.4...: .orange
        default: .red
        }
    }

    // MARK: - Category Chips

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
                    .background(Capsule().fill(info.color.opacity(0.12)))
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

}

// MARK: - Route Score Color

extension Route {
    var scoreColor: Color {
        switch score {
        case 7...: .green
        case 4...: .orange
        default: .red
        }
    }
}
