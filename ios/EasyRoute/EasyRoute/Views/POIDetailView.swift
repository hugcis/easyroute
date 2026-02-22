import SwiftUI

struct POIDetailView: View {
    let poi: SelectedPoi
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            if let desc = poi.description, !desc.isEmpty {
                Text(desc)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            infoChips
        }
        .padding()
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: - Header

    private var header: some View {
        let info = POICategories.info(for: poi.category)
        return HStack(spacing: 10) {
            Circle()
                .fill(info.color)
                .frame(width: 36, height: 36)
                .overlay {
                    Image(systemName: info.symbol)
                        .font(.system(size: 16))
                        .foregroundStyle(.white)
                }

            VStack(alignment: .leading, spacing: 2) {
                Text(poi.name)
                    .font(.headline)
                    .lineLimit(1)
                HStack(spacing: 6) {
                    Text(info.label)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    badge
                }
            }

            Spacer()

            Button(action: onDismiss) {
                Image(systemName: "xmark.circle.fill")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var badge: some View {
        if let order = poi.orderInRoute {
            Text("Stop #\(order)")
                .font(.caption2.bold())
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(Capsule().fill(.blue.opacity(0.15)))
                .foregroundStyle(.blue)
        } else {
            Text("Nearby")
                .font(.caption2.bold())
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(Capsule().fill(.orange.opacity(0.15)))
                .foregroundStyle(.orange)
        }
    }

    // MARK: - Info chips

    private var infoChips: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                if let minutes = poi.estimatedVisitDurationMinutes {
                    chipView(icon: "clock", text: "\(minutes) min visit")
                }
                chipView(
                    icon: "figure.walk",
                    text: String(format: "%.1f km from start", poi.distanceFromStartKm)
                )
                if let dist = poi.distanceFromPathM {
                    chipView(
                        icon: "arrow.left.and.right",
                        text: String(format: "%.0f m from path", dist)
                    )
                }
                chipView(
                    icon: "star",
                    text: String(format: "%.1f", poi.popularityScore)
                )
            }
        }
    }

    private func chipView(icon: String, text: String) -> some View {
        HStack(spacing: 4) {
            Image(systemName: icon)
                .font(.caption2)
            Text(text)
                .font(.caption)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .background(Capsule().fill(.quaternary))
    }
}
