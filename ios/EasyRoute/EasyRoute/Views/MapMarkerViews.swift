import SwiftUI

struct StartMarkerView: View {
    var body: some View {
        Circle()
            .fill(.green)
            .stroke(.white, lineWidth: 2)
            .frame(width: 16, height: 16)
            .shadow(radius: 2)
    }
}

struct WaypointMarkerView: View {
    let order: Int
    let category: String

    var body: some View {
        let info = POICategories.info(for: category)
        ZStack {
            Circle()
                .fill(info.color)
                .frame(width: 28, height: 28)
            Text("\(order)")
                .font(.caption.bold())
                .foregroundStyle(.white)
        }
        .overlay(Circle().stroke(.white, lineWidth: 2))
        .shadow(radius: 2)
    }
}

struct SnappedMarkerView: View {
    let category: String

    var body: some View {
        let info = POICategories.info(for: category)
        Image(systemName: info.symbol)
            .font(.caption2)
            .foregroundStyle(.white)
            .padding(4)
            .background(Circle().fill(info.color.opacity(0.8)))
            .overlay(Circle().stroke(.white, lineWidth: 1))
            .shadow(radius: 1)
    }
}
