# POI Categories Reference

Complete list of Point of Interest (POI) categories supported by EasyRoute.

## Usage

Include categories in your route request using the `poi_categories` preference:

```json
{
  "start_point": {"lat": 48.8566, "lng": 2.3522},
  "distance_km": 5.0,
  "mode": "walk",
  "preferences": {
    "poi_categories": ["monument", "viewpoint", "park"]
  }
}
```

If no categories are specified, **all categories** are searched.

---

## Original Categories

### `monument`
War memorials, statues, and commemorative monuments.

**OSM Tags:** `tourism=monument`, `tourism=memorial`

**Good for:** Historical walks, cultural tours

**Examples:** Eiffel Tower, Arc de Triomphe, war memorials

---

### `viewpoint`
Scenic overlooks and observation points.

**OSM Tags:** `tourism=viewpoint`

**Good for:** Scenic routes, photography walks

**Examples:** Montmartre viewpoint, mountain lookouts

---

### `park`
Public parks, gardens, and green spaces.

**OSM Tags:** `leisure=park`, `leisure=garden`

**Good for:** Relaxing walks, nature routes

**Examples:** Luxembourg Gardens, Central Park

---

### `museum`
Art museums, history museums, and galleries.

**OSM Tags:** `tourism=museum`, `tourism=gallery`

**Good for:** Cultural routes, rainy day alternatives

**Examples:** Louvre Museum, Musée d'Orsay

---

### `restaurant`
Dining establishments and eateries.

**OSM Tags:** `amenity=restaurant`

**Good for:** Lunch routes, foodie tours

**Examples:** Local bistros, fine dining

---

### `cafe`
Coffee shops and cafés.

**OSM Tags:** `amenity=cafe`

**Good for:** Coffee breaks, short walks

**Examples:** Parisian cafés, coffee houses

---

### `historic`
Broad category for historical sites (churches, castles, ruins, wayside crosses, etc.)

**OSM Tags:** `historic=*` (any historic tag)

**Good for:** Historical walks, educational routes

**Examples:** Ancient ruins, historic buildings, wayside crosses, old bridges

**Note:** This is a catch-all category. For specific types, use `church`, `castle`, etc.

---

### `cultural`
Cultural attractions and arts centers.

**OSM Tags:** `tourism=attraction`, `amenity=arts_centre`

**Good for:** Cultural exploration, art walks

**Examples:** Cultural centers, public art installations

---

## Natural & Scenic Categories

### `waterfront`
Beaches, coastlines, and waterfront areas.

**OSM Tags:** `natural=beach`, `natural=coastline`, `leisure=beach_resort`

**Good for:** Coastal walks, beach routes

**Examples:** Sandy beaches, coastal promenades

---

### `waterfall`
Waterfalls and cascades.

**OSM Tags:** `waterway=waterfall`

**Good for:** Nature walks, scenic hikes

**Examples:** Natural waterfalls, cascades

---

### `nature_reserve`
Protected natural areas and nature reserves.

**OSM Tags:** `leisure=nature_reserve`, `boundary=protected_area`

**Good for:** Nature exploration, wildlife watching

**Examples:** National parks, wildlife sanctuaries

---

## Architectural Categories

### `church`
Religious buildings, churches, cathedrals.

**OSM Tags:** `amenity=place_of_worship`, `building=church`, `building=cathedral`

**Good for:** Architectural tours, religious heritage routes

**Examples:** Notre-Dame, local churches, cathedrals

---

### `castle`
Castles, forts, and fortifications.

**OSM Tags:** `historic=castle`, `historic=fort`, `historic=fortress`

**Good for:** Medieval history tours, fortress routes

**Examples:** Loire Valley châteaux, medieval castles

**Regional Highlight:** Perfect for Loire Valley routes!

---

### `bridge`
Notable bridges and architectural crossings.

**OSM Tags:** `man_made=bridge`, `bridge=yes`

**Good for:** Engineering appreciation, urban walks

**Examples:** Pont Neuf, historic bridges

---

### `tower`
Towers, bell towers, and observation towers.

**OSM Tags:** `man_made=tower`, `tower:type=bell_tower`

**Good for:** Architectural routes, urban landmarks

**Examples:** Bell towers, communication towers

---

## Urban Interest Categories

### `plaza`
Town squares and public plazas.

**OSM Tags:** `place=square`, `leisure=plaza`

**Good for:** Urban exploration, social spaces

**Examples:** Place de la Concorde, village squares

---

### `fountain`
Decorative and historic fountains.

**OSM Tags:** `amenity=fountain`

**Good for:** Urban walks, photo opportunities

**Examples:** Trevi Fountain, public fountains

---

### `market`
Local markets and marketplaces.

**OSM Tags:** `amenity=marketplace`, `shop=marketplace`

**Good for:** Cultural immersion, shopping walks

**Examples:** Farmers markets, local bazaars

---

### `artwork`
Public art and sculptures.

**OSM Tags:** `tourism=artwork`, `artwork_type=*`

**Good for:** Art walks, cultural routes

**Examples:** Street art, public sculptures

---

### `lighthouse`
Lighthouses and maritime landmarks.

**OSM Tags:** `man_made=lighthouse`

**Good for:** Coastal routes, maritime heritage

**Examples:** Coastal lighthouses, harbor markers

---

## Activity Categories

### `winery`
Wineries, wine cellars, and wine shops.

**OSM Tags:** `craft=winery`, `shop=wine`, `tourism=wine_cellar`

**Good for:** Wine routes, vineyard tours

**Examples:** Loire Valley wineries, Champagne cellars

**Regional Highlight:** Perfect for Loire Valley wine routes!

---

### `brewery`
Breweries and craft beer locations.

**OSM Tags:** `craft=brewery`, `industrial=brewery`

**Good for:** Beer routes, brewery tours

**Examples:** Craft breweries, historic brewing sites

---

### `theatre`
Theatres, cinemas, and performance venues.

**OSM Tags:** `amenity=theatre`, `amenity=cinema`

**Good for:** Cultural routes, entertainment walks

**Examples:** Opera houses, local theatres

---

### `library`
Public and historic libraries.

**OSM Tags:** `amenity=library`

**Good for:** Educational routes, architectural tours

**Examples:** National libraries, historic book collections

---

## Category Combinations

### Recommended Combinations

**Classic Tourist Route:**
```json
"poi_categories": ["monument", "viewpoint", "museum", "church"]
```

**Nature Walk:**
```json
"poi_categories": ["park", "waterfront", "nature_reserve", "waterfall"]
```

**Architectural Tour:**
```json
"poi_categories": ["church", "castle", "bridge", "tower", "historic"]
```

**Cultural Exploration:**
```json
"poi_categories": ["museum", "cultural", "artwork", "theatre", "plaza"]
```

**Loire Valley Route:**
```json
"poi_categories": ["castle", "church", "winery", "historic"]
```

**Coastal Walk:**
```json
"poi_categories": ["waterfront", "lighthouse", "viewpoint", "beach"]
```

**Urban Discovery:**
```json
"poi_categories": ["plaza", "fountain", "market", "artwork", "cafe"]
```

**Foodie Tour:**
```json
"poi_categories": ["restaurant", "cafe", "market", "winery", "brewery"]
```

---

## Tips

1. **Fewer is Better**: 3-5 categories usually gives best results
2. **Regional Variations**: Some categories work better in certain regions
   - `castle` and `winery` are great for Loire Valley
   - `waterfront` and `lighthouse` for coastal areas
   - `church` is excellent throughout France
3. **Hidden Gems**: Set `"hidden_gems": true` to prefer less popular POIs
4. **Density Matters**: Dense areas (cities) can support more specific categories

---

## Data Source

All POI data comes from **OpenStreetMap** via the Overpass API. Coverage and quality depend on OSM contributors in your area.

---

## Total Categories

**24 categories** available:
- 8 Original categories
- 3 Natural/Scenic
- 4 Architectural
- 5 Urban Interest
- 4 Activity categories
