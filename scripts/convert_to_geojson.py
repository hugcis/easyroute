#!/usr/bin/env python3
"""
Convert EasyRoute API response to GeoJSON for visualization.
Usage: python scripts/convert_to_geojson.py < route_response.json > route.geojson
Or: curl ... | python scripts/convert_to_geojson.py > route.geojson
"""

import json
import sys

def convert_to_geojson(api_response):
    """Convert API response to GeoJSON FeatureCollection"""

    features = []

    for idx, route in enumerate(api_response.get('routes', [])):
        # Convert path to GeoJSON LineString (note: GeoJSON uses [lng, lat] order!)
        line_coordinates = [[coord['lng'], coord['lat']] for coord in route['path']]

        # Add route path as LineString
        features.append({
            "type": "Feature",
            "properties": {
                "type": "route",
                "route_index": idx,
                "distance_km": route['distance_km'],
                "duration_minutes": route['estimated_duration_minutes'],
                "score": route['score'],
                "name": f"Route {idx + 1} ({route['distance_km']:.1f}km, score: {route['score']:.1f})"
            },
            "geometry": {
                "type": "LineString",
                "coordinates": line_coordinates
            }
        })

        # Add waypoint POIs as Point features
        for poi in route['pois']:
            features.append({
                "type": "Feature",
                "properties": {
                    "type": "poi_waypoint",
                    "route_index": idx,
                    "name": poi['name'],
                    "category": poi['category'],
                    "popularity_score": poi['popularity_score'],
                    "order_in_route": poi['order_in_route'],
                    "distance_from_start_km": poi['distance_from_start_km'],
                    "description": poi.get('description', ''),
                },
                "geometry": {
                    "type": "Point",
                    "coordinates": [poi['coordinates']['lng'], poi['coordinates']['lat']]
                }
            })

        # Add snapped POIs (not used as waypoints)
        for poi in route.get('snapped_pois', []):
            features.append({
                "type": "Feature",
                "properties": {
                    "type": "poi_snapped",
                    "route_index": idx,
                    "name": poi['name'],
                    "category": poi['category'],
                    "popularity_score": poi['popularity_score'],
                    "distance_from_start_km": poi['distance_from_start_km'],
                    "distance_from_path_m": poi['distance_from_path_m'],
                    "description": poi.get('description', ''),
                },
                "geometry": {
                    "type": "Point",
                    "coordinates": [poi['coordinates']['lng'], poi['coordinates']['lat']]
                }
            })

        # Add start point marker
        if route['path']:
            start = route['path'][0]
            features.append({
                "type": "Feature",
                "properties": {
                    "type": "start",
                    "route_index": idx,
                    "name": "Start/End"
                },
                "geometry": {
                    "type": "Point",
                    "coordinates": [start['lng'], start['lat']]
                }
            })

    return {
        "type": "FeatureCollection",
        "features": features
    }

if __name__ == "__main__":
    try:
        # Read from stdin
        api_response = json.load(sys.stdin)

        # Convert to GeoJSON
        geojson = convert_to_geojson(api_response)

        # Output to stdout
        print(json.dumps(geojson, indent=2))

    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON input - {e}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
