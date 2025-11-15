# Route Visualization Guide

## What the API Returns

The API returns routes with:

1. **Path**: Array of lat/lng coordinates forming the complete route
   - This is the actual turn-by-turn path from Mapbox
   - Can be 100+ coordinates for detailed routes

2. **POIs**: Points of interest along the route with their coordinates

3. **Metadata**: Distance, duration, score, etc.

## Visualization Methods

### Method 1: Interactive HTML Visualizer (Recommended)

1. **Set up the visualizer:**
   ```bash
   # Edit scripts/visualize.html and add your Mapbox token
   # Line 87: mapboxgl.accessToken = 'YOUR_MAPBOX_TOKEN_HERE';
   ```

2. **Get a route from the API:**
   ```bash
   curl -X POST http://localhost:3000/api/v1/routes/loop \
     -H "Content-Type: application/json" \
     -d @examples/test_request.json > route_response.json
   ```

3. **Open the visualizer:**
   ```bash
   # Open scripts/visualize.html in your browser
   # Copy the contents of route_response.json
   # Paste into the text area and click "Visualize Routes"
   ```

**Features:**
- Interactive map with zoom/pan
- Click POI markers for details
- Switch between route alternatives
- Shows route path, POIs, and start/end point

---

### Method 2: geojson.io (Quick & Easy)

1. **Convert API response to GeoJSON:**
   ```bash
   curl -X POST http://localhost:3000/api/v1/routes/loop \
     -H "Content-Type: application/json" \
     -d @examples/test_request.json \
     | python3 scripts/convert_to_geojson.py > route.geojson
   ```

2. **Visualize:**
   - Go to https://geojson.io
   - Click "Open" → "File" → Select `route.geojson`
   - Your route will be displayed with POI markers!

**Features:**
- No setup required
- Edit routes visually
- Export to various formats
- Share via URL

---

### Method 3: Mapbox Studio

1. **Convert to GeoJSON** (same as above)

2. **Upload to Mapbox:**
   - Go to https://studio.mapbox.com/
   - Upload `route.geojson` as a dataset
   - Style and customize

---

### Method 4: Python Visualization with Folium

```python
import json
import folium

# Load route response
with open('route_response.json') as f:
    data = json.load(f)

route = data['routes'][0]

# Create map centered on start point
start = route['path'][0]
m = folium.Map(location=[start['lat'], start['lng']], zoom_start=13)

# Add route path
path_coords = [[coord['lat'], coord['lng']] for coord in route['path']]
folium.PolyLine(
    path_coords,
    color='blue',
    weight=4,
    opacity=0.7
).add_to(m)

# Add POI markers
for poi in route['pois']:
    folium.Marker(
        [poi['coordinates']['lat'], poi['coordinates']['lng']],
        popup=f"<b>{poi['name']}</b><br>{poi['category']}",
        icon=folium.Icon(color='red', icon='info-sign')
    ).add_to(m)

# Add start marker
folium.Marker(
    [start['lat'], start['lng']],
    popup='Start/End',
    icon=folium.Icon(color='green', icon='play')
).add_to(m)

# Save
m.save('route_map.html')
```

---

## Response Format Details

### Full JSON Structure:

```json
{
  "routes": [                           // Multiple alternatives
    {
      "id": "uuid",
      "distance_km": 5.2,
      "estimated_duration_minutes": 78,
      "elevation_gain_m": null,
      "score": 8.5,                    // 0-10 quality score

      "path": [                         // Complete route path
        {"lat": 48.8566, "lng": 2.3522},
        {"lat": 48.8567, "lng": 2.3524},
        // ... 100+ more coordinates
        {"lat": 48.8566, "lng": 2.3522}  // Loop back to start
      ],

      "pois": [                          // Points of interest
        {
          "id": "uuid",
          "name": "Eiffel Tower",
          "category": "monument",
          "coordinates": {"lat": 48.8584, "lng": 2.2945},
          "popularity_score": 95.0,      // 0-100
          "description": "Iconic tower",
          "estimated_visit_duration_minutes": 120,
          "order_in_route": 1,
          "distance_from_start_km": 1.7
        }
      ]
    }
  ]
}
```

### Coordinate Format Notes:

- **API uses:** `{lat: ..., lng: ...}`
- **GeoJSON uses:** `[lng, lat]` (reversed!)
- **Leaflet uses:** `[lat, lng]`
- **Mapbox GL JS uses:** `[lng, lat]` (like GeoJSON)

The conversion script handles this automatically.

---

## Quick Test Example

```bash
# 1. Start the server
cargo run

# 2. Generate a route (use Paris coordinates)
curl -X POST http://localhost:3000/api/v1/routes/loop \
  -H "Content-Type: application/json" \
  -d '{
    "start_point": {"lat": 48.8566, "lng": 2.3522},
    "distance_km": 3.0,
    "mode": "walk",
    "preferences": {"max_alternatives": 1}
  }' | python3 -m json.tool

# 3. Convert to GeoJSON and view
curl -X POST http://localhost:3000/api/v1/routes/loop \
  -H "Content-Type: application/json" \
  -d @examples/test_request.json \
  | python3 scripts/convert_to_geojson.py \
  | pbcopy  # Copies to clipboard (macOS)

# 4. Paste into https://geojson.io
```

---

## What You'll See

When visualized, you'll see:

1. **Blue line** - The complete walking/biking route
2. **Numbered markers** - POIs in order (1, 2, 3...)
3. **Green marker** - Start/End point
4. **Multiple routes** - If you requested alternatives (different colors)

Each POI marker is clickable and shows:
- Name and category
- Popularity score
- Description (if available)
- Distance from start

---

## Troubleshooting

**No route displayed?**
- Check that POIs exist in the area (first request fetches from Overpass API, may be slow)
- Try a different location (Paris works well: 48.8566, 2.3522)
- Check API logs for errors

**Mapbox token error?**
- Get a free token at https://account.mapbox.com/
- Replace `YOUR_MAPBOX_TOKEN_HERE` in visualize.html

**Empty path?**
- The Mapbox API may have failed
- Check your Mapbox API key in .env
- Check you haven't exceeded free tier (100k requests/month)
