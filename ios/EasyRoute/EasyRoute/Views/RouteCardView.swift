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
            }
            .foregroundStyle(.secondary)

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

    private var scoreColor: Color {
        if route.score >= 7 { return .green }
        if route.score >= 4 { return .orange }
        return .red
    }
}
