import SwiftUI

struct CompactRouteSwitcherView: View {
    var routeState: RouteState

    private var route: Route? { routeState.selectedRoute }
    private var count: Int { routeState.routes.count }
    private var index: Int { routeState.selectedRouteIndex }

    var body: some View {
        if let route {
            VStack(spacing: 8) {
                HStack(spacing: 12) {
                    chevronButton(direction: .left)
                        .opacity(count > 1 ? 1 : 0)

                    routeInfoCapsule(route)

                    chevronButton(direction: .right)
                        .opacity(count > 1 ? 1 : 0)
                }

                if count > 1 {
                    pageDots
                }
            }
            .animation(.snappy(duration: 0.25), value: index)
            .highPriorityGesture(
                DragGesture(minimumDistance: 20)
                    .onEnded { value in
                        withAnimation(.snappy(duration: 0.25)) {
                            if value.translation.width < -20, index < count - 1 {
                                routeState.selectedRouteIndex += 1
                            } else if value.translation.width > 20, index > 0 {
                                routeState.selectedRouteIndex -= 1
                            }
                        }
                    }
            )
        }
    }

    // MARK: - Route Info Capsule

    private func routeInfoCapsule(_ route: Route) -> some View {
        HStack(spacing: 10) {
            Text(String(format: "%.1f", route.score))
                .font(.subheadline.bold().monospacedDigit())
                .foregroundStyle(route.scoreColor)
                .contentTransition(.numericText(value: Double(route.score)))

            statLabel(icon: "arrow.triangle.swap", text: String(format: "%.1f km", route.distanceKm))
                .contentTransition(.numericText(value: route.distanceKm))

            statLabel(icon: "clock", text: "\(route.estimatedDurationMinutes) min")
                .contentTransition(.numericText(value: Double(route.estimatedDurationMinutes)))

            statLabel(icon: "mappin", text: "\(route.pois.count)")
                .contentTransition(.numericText(value: Double(route.pois.count)))
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(.regularMaterial, in: Capsule())
    }

    private func statLabel(icon: String, text: String) -> some View {
        HStack(spacing: 2) {
            Image(systemName: icon)
                .font(.caption2)
            Text(text)
                .font(.caption.monospacedDigit())
        }
        .foregroundStyle(.secondary)
    }

    // MARK: - Chevron Buttons

    private enum Direction { case left, right }

    private func chevronButton(direction: Direction) -> some View {
        let isLeft = direction == .left
        return Button {
            withAnimation(.snappy(duration: 0.25)) {
                routeState.selectedRouteIndex += isLeft ? -1 : 1
            }
        } label: {
            Image(systemName: isLeft ? "chevron.left" : "chevron.right")
                .font(.caption.bold())
                .foregroundStyle(.secondary)
                .frame(width: 28, height: 28)
                .contentShape(Rectangle())
        }
        .disabled(isLeft ? index <= 0 : index >= count - 1)
    }

    // MARK: - Page Dots

    private var pageDots: some View {
        HStack(spacing: 6) {
            ForEach(0..<count, id: \.self) { i in
                Circle()
                    .fill(i == index ? Color.accentColor : Color.secondary.opacity(0.3))
                    .frame(width: 6, height: 6)
                    .scaleEffect(i == index ? 1.2 : 1.0)
            }
        }
    }
}
