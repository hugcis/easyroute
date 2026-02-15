import SwiftUI

enum GPXExporter {
    static func generate(route: Route) -> Data {
        let name = "EasyRoute Loop — \(String(format: "%.1f", route.distanceKm)) km"

        var gpx = """
        <?xml version="1.0" encoding="UTF-8"?>
        <gpx version="1.1" creator="EasyRoute"
          xmlns="http://www.topografix.com/GPX/1/1"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
          xsi:schemaLocation="http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd">
          <metadata><name>\(escapeXML(name))</name></metadata>\n
        """

        // Waypoint POIs
        for poi in route.pois {
            gpx += """
              <wpt lat="\(poi.coordinates.lat)" lon="\(poi.coordinates.lng)">
                <name>\(escapeXML(poi.name))</name>
                <desc>\(escapeXML("Waypoint #\(poi.orderInRoute) — \(poi.category)"))</desc>
              </wpt>\n
            """
        }

        // Snapped POIs
        for poi in route.snappedPois {
            gpx += """
              <wpt lat="\(poi.coordinates.lat)" lon="\(poi.coordinates.lng)">
                <name>\(escapeXML(poi.name))</name>
                <desc>\(escapeXML("Nearby — \(poi.category)"))</desc>
              </wpt>\n
            """
        }

        // Track
        gpx += "  <trk><name>\(escapeXML(name))</name><trkseg>\n"
        for point in route.path {
            gpx += "    <trkpt lat=\"\(point.lat)\" lon=\"\(point.lng)\"></trkpt>\n"
        }
        gpx += "  </trkseg></trk>\n</gpx>\n"

        return Data(gpx.utf8)
    }

    static func shareGPX(route: Route) -> URL? {
        let data = generate(route: route)
        let filename = "easyroute-\(String(format: "%.1f", route.distanceKm))km.gpx"
        let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent(filename)
        do {
            try data.write(to: tempURL)
            return tempURL
        } catch {
            return nil
        }
    }

    private static func escapeXML(_ string: String) -> String {
        string
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "'", with: "&apos;")
    }
}

struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}
