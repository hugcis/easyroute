import SwiftUI

struct CategoryInfo {
    let label: String
    let color: Color
    let hexColor: String
    let symbol: String
}

struct CategoryGroup: Identifiable {
    let id: String
    let name: String
    let categories: [(key: String, info: CategoryInfo)]
}

enum POICategories {
    static let groups: [CategoryGroup] = [
        CategoryGroup(id: "original", name: "Popular", categories: [
            ("monument", CategoryInfo(label: "Monument", color: Color(hex: 0xe74c3c), hexColor: "#e74c3c", symbol: "building.columns")),
            ("viewpoint", CategoryInfo(label: "Viewpoint", color: Color(hex: 0xe67e22), hexColor: "#e67e22", symbol: "binoculars")),
            ("park", CategoryInfo(label: "Park", color: Color(hex: 0x27ae60), hexColor: "#27ae60", symbol: "leaf")),
            ("museum", CategoryInfo(label: "Museum", color: Color(hex: 0x8e44ad), hexColor: "#8e44ad", symbol: "building.columns.fill")),
            ("restaurant", CategoryInfo(label: "Restaurant", color: Color(hex: 0xd35400), hexColor: "#d35400", symbol: "fork.knife")),
            ("cafe", CategoryInfo(label: "Cafe", color: Color(hex: 0xa0522d), hexColor: "#a0522d", symbol: "cup.and.saucer")),
            ("historic", CategoryInfo(label: "Historic", color: Color(hex: 0xc0392b), hexColor: "#c0392b", symbol: "clock.arrow.circlepath")),
            ("cultural", CategoryInfo(label: "Cultural", color: Color(hex: 0x9b59b6), hexColor: "#9b59b6", symbol: "theatermasks")),
        ]),
        CategoryGroup(id: "natural", name: "Natural / Scenic", categories: [
            ("waterfront", CategoryInfo(label: "Waterfront", color: Color(hex: 0x2980b9), hexColor: "#2980b9", symbol: "water.waves")),
            ("waterfall", CategoryInfo(label: "Waterfall", color: Color(hex: 0x1abc9c), hexColor: "#1abc9c", symbol: "drop")),
            ("nature_reserve", CategoryInfo(label: "Nature Reserve", color: Color(hex: 0x16a085), hexColor: "#16a085", symbol: "tree")),
        ]),
        CategoryGroup(id: "architectural", name: "Architectural", categories: [
            ("church", CategoryInfo(label: "Church", color: Color(hex: 0x7f8c8d), hexColor: "#7f8c8d", symbol: "cross")),
            ("castle", CategoryInfo(label: "Castle", color: Color(hex: 0x8B4513), hexColor: "#8B4513", symbol: "building")),
            ("bridge", CategoryInfo(label: "Bridge", color: Color(hex: 0x607D8B), hexColor: "#607D8B", symbol: "road.lanes")),
            ("tower", CategoryInfo(label: "Tower", color: Color(hex: 0x795548), hexColor: "#795548", symbol: "arrow.up.to.line")),
        ]),
        CategoryGroup(id: "urban", name: "Urban Interest", categories: [
            ("plaza", CategoryInfo(label: "Plaza", color: Color(hex: 0xf39c12), hexColor: "#f39c12", symbol: "square.grid.2x2")),
            ("fountain", CategoryInfo(label: "Fountain", color: Color(hex: 0x3498db), hexColor: "#3498db", symbol: "drop.circle")),
            ("market", CategoryInfo(label: "Market", color: Color(hex: 0xe67e22), hexColor: "#e67e22", symbol: "cart")),
            ("artwork", CategoryInfo(label: "Artwork", color: Color(hex: 0xe91e63), hexColor: "#e91e63", symbol: "paintpalette")),
            ("lighthouse", CategoryInfo(label: "Lighthouse", color: Color(hex: 0xffc107), hexColor: "#ffc107", symbol: "light.beacon.max")),
        ]),
        CategoryGroup(id: "activity", name: "Activity", categories: [
            ("winery", CategoryInfo(label: "Winery", color: Color(hex: 0x722f37), hexColor: "#722f37", symbol: "wineglass")),
            ("brewery", CategoryInfo(label: "Brewery", color: Color(hex: 0xd4a017), hexColor: "#d4a017", symbol: "mug")),
            ("theatre", CategoryInfo(label: "Theatre", color: Color(hex: 0xad1457), hexColor: "#ad1457", symbol: "theatermasks.fill")),
            ("library", CategoryInfo(label: "Library", color: Color(hex: 0x5c6bc0), hexColor: "#5c6bc0", symbol: "books.vertical")),
        ]),
    ]

    static let allCategories: [String: CategoryInfo] = {
        var dict: [String: CategoryInfo] = [:]
        for group in groups {
            for (key, info) in group.categories {
                dict[key] = info
            }
        }
        return dict
    }()

    static let allCategoryKeys: Set<String> = {
        Set(allCategories.keys)
    }()

    static func info(for category: String) -> CategoryInfo {
        allCategories[category] ?? CategoryInfo(
            label: category.capitalized,
            color: .gray,
            hexColor: "#888888",
            symbol: "mappin"
        )
    }
}

// MARK: - Color hex initializer

extension Color {
    init(hex: UInt, opacity: Double = 1.0) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255.0,
            green: Double((hex >> 8) & 0xFF) / 255.0,
            blue: Double(hex & 0xFF) / 255.0,
            opacity: opacity
        )
    }
}
