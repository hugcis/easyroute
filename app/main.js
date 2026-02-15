// ── Configuration ─────────────────────────────────────────
// API base URL: empty string means relative to current origin (works on localhost)
const API_BASE = '';

// ── Category data ──────────────────────────────────────────
const POI_CATEGORIES = {
    'Original': {
        monument: { label: 'Monument', color: '#e74c3c' },
        viewpoint: { label: 'Viewpoint', color: '#e67e22' },
        park: { label: 'Park', color: '#27ae60' },
        museum: { label: 'Museum', color: '#8e44ad' },
        restaurant: { label: 'Restaurant', color: '#d35400' },
        cafe: { label: 'Cafe', color: '#a0522d' },
        historic: { label: 'Historic', color: '#c0392b' },
        cultural: { label: 'Cultural', color: '#9b59b6' },
    },
    'Natural / Scenic': {
        waterfront: { label: 'Waterfront', color: '#2980b9' },
        waterfall: { label: 'Waterfall', color: '#1abc9c' },
        nature_reserve: { label: 'Nature Reserve', color: '#16a085' },
    },
    'Architectural': {
        church: { label: 'Church', color: '#7f8c8d' },
        castle: { label: 'Castle', color: '#8B4513' },
        bridge: { label: 'Bridge', color: '#607D8B' },
        tower: { label: 'Tower', color: '#795548' },
    },
    'Urban Interest': {
        plaza: { label: 'Plaza', color: '#f39c12' },
        fountain: { label: 'Fountain', color: '#3498db' },
        market: { label: 'Market', color: '#e67e22' },
        artwork: { label: 'Artwork', color: '#e91e63' },
        lighthouse: { label: 'Lighthouse', color: '#ffc107' },
    },
    'Activity': {
        winery: { label: 'Winery', color: '#722f37' },
        brewery: { label: 'Brewery', color: '#d4a017' },
        theatre: { label: 'Theatre', color: '#ad1457' },
        library: { label: 'Library', color: '#5c6bc0' },
    },
};

const CATEGORY_INFO = {};
for (const cats of Object.values(POI_CATEGORIES)) Object.assign(CATEGORY_INFO, cats);

const selectedCats = new Set(); // empty = all categories
let catPopoverOpen = false;

function buildCatPills() {
    const el = document.getElementById('catPills');
    for (const [group, cats] of Object.entries(POI_CATEGORIES)) {
        const g = document.createElement('span');
        g.className = 'cat-group-name'; g.textContent = group;
        el.appendChild(g);
        for (const [key, info] of Object.entries(cats)) {
            const p = document.createElement('span');
            p.className = 'cat-pill'; p.textContent = info.label;
            p.dataset.cat = key;
            p.style.borderColor = info.color; p.style.color = info.color;
            p.onclick = () => toggleCat(key, p);
            el.appendChild(p);
        }
    }
}

function toggleCat(key, pill) {
    const info = CATEGORY_INFO[key];
    if (selectedCats.has(key)) {
        selectedCats.delete(key);
        pill.classList.remove('on'); pill.style.background = 'white'; pill.style.color = info.color;
    } else {
        selectedCats.add(key);
        pill.classList.add('on'); pill.style.background = info.color; pill.style.color = 'white';
    }
    updateCatLabel();
}

function selectAllCats() {
    document.querySelectorAll('#catPills .cat-pill').forEach(p => {
        const info = CATEGORY_INFO[p.dataset.cat]; selectedCats.add(p.dataset.cat);
        p.classList.add('on'); p.style.background = info.color; p.style.color = 'white';
    });
    updateCatLabel();
}

function clearAllCats() {
    document.querySelectorAll('#catPills .cat-pill').forEach(p => {
        const info = CATEGORY_INFO[p.dataset.cat]; selectedCats.delete(p.dataset.cat);
        p.classList.remove('on'); p.style.background = 'white'; p.style.color = info.color;
    });
    updateCatLabel();
}

function updateCatLabel() {
    const n = selectedCats.size, total = Object.keys(CATEGORY_INFO).length;
    const label = document.getElementById('catLabel');
    if (n === 0 || n === total) label.textContent = 'All categories';
    else if (n <= 2) label.textContent = [...selectedCats].map(k => CATEGORY_INFO[k].label).join(', ');
    else label.textContent = n + ' categories selected';
}

function toggleCatPopover() {
    catPopoverOpen = !catPopoverOpen;
    const popover = document.getElementById('catPopover');
    const arrow = document.getElementById('catArrow');
    popover.classList.toggle('open', catPopoverOpen);
    arrow.classList.toggle('open', catPopoverOpen);
    if (catPopoverOpen) {
        const rect = document.querySelector('.cat-trigger').getBoundingClientRect();
        popover.style.top = (rect.bottom + 4) + 'px';
        popover.style.left = rect.left + 'px';
        popover.style.width = rect.width + 'px';
    }
}

document.addEventListener('click', e => {
    if (catPopoverOpen && !document.getElementById('catDropdown').contains(e.target)) {
        catPopoverOpen = false;
        document.getElementById('catPopover').classList.remove('open');
        document.getElementById('catArrow').classList.remove('open');
    }
});

// ── Map setup ──────────────────────────────────────────────
mapboxgl.accessToken = window.MAPBOX_TOKEN || prompt('Enter your Mapbox access token:');
const map = new mapboxgl.Map({
    container: 'map', style: 'mapbox://styles/mapbox/streets-v12',
    center: [2.3522, 48.8566], zoom: 12
});
map.addControl(new mapboxgl.NavigationControl());

let currentMarkers = [], currentRoutes = null, selectedRouteIndex = 0;
let hiddenCategories = new Set();

// ── Sync coordinates with map center ───────────────────────
let syncEnabled = true, _suppressSync = false;

function toggleSync() {
    syncEnabled = !syncEnabled;
    document.getElementById('syncToggle').classList.toggle('on', syncEnabled);
    if (syncEnabled) syncCoordsFromMap();
}

function syncCoordsFromMap() {
    if (!syncEnabled || _suppressSync) return;
    const c = map.getCenter();
    document.getElementById('startLat').value = c.lat.toFixed(4);
    document.getElementById('startLng').value = c.lng.toFixed(4);
}

function updateMapCenter() {
    const c = map.getCenter();
    document.getElementById('mapCenterDisplay').textContent = c.lat.toFixed(4) + ', ' + c.lng.toFixed(4);
    syncCoordsFromMap();
}
map.on('move', updateMapCenter);

// Disable sync on manual coordinate edit
['startLat', 'startLng'].forEach(id => {
    document.getElementById(id).addEventListener('input', () => {
        if (syncEnabled) { syncEnabled = false; document.getElementById('syncToggle').classList.remove('on'); }
    });
});

// ── Geolocation ────────────────────────────────────────────
function useMyLocation() {
    if (!navigator.geolocation) {
        alert('Geolocation is not supported by your browser');
        return;
    }
    const btn = document.getElementById('geolocateBtn');
    btn.textContent = 'Locating...';
    btn.disabled = true;
    navigator.geolocation.getCurrentPosition(
        pos => {
            const lat = pos.coords.latitude;
            const lng = pos.coords.longitude;
            document.getElementById('startLat').value = lat.toFixed(4);
            document.getElementById('startLng').value = lng.toFixed(4);
            // Disable sync and pan map to location
            syncEnabled = false;
            document.getElementById('syncToggle').classList.remove('on');
            _suppressSync = true;
            map.flyTo({ center: [lng, lat], zoom: 14 });
            map.once('moveend', () => { _suppressSync = false; });
            btn.textContent = 'Use my location';
            btn.disabled = false;
        },
        err => {
            alert('Could not get your location: ' + err.message);
            btn.textContent = 'Use my location';
            btn.disabled = false;
        },
        { enableHighAccuracy: true, timeout: 10000 }
    );
}

// ── UI helpers ─────────────────────────────────────────────
function switchTab(tab) {
    document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
    event.target.classList.add('active');
    document.getElementById('generateTab').classList.toggle('active', tab === 'generate');
    document.getElementById('pasteTab').classList.toggle('active', tab !== 'generate');
}

function mkEl(cls, color, text) {
    const el = document.createElement('div');
    el.className = cls;
    if (color) el.style.backgroundColor = color;
    if (text !== undefined) el.textContent = text;
    return el;
}

// ── Generate route via API ─────────────────────────────────
async function generateRoute() {
    const btn = document.getElementById('generateBtn');
    const err = document.getElementById('errorMsg');
    const panel = document.querySelector('.panel');
    err.style.display = 'none'; err.className = '';
    btn.disabled = true; btn.textContent = 'Generating...'; panel.classList.add('loading');

    try {
        const lat = parseFloat(document.getElementById('startLat').value);
        const lng = parseFloat(document.getElementById('startLng').value);
        const dist = parseFloat(document.getElementById('distance').value);
        const mode = document.getElementById('mode').value;
        if (isNaN(lat) || isNaN(lng)) throw new Error('Invalid start coordinates');
        if (isNaN(dist) || dist <= 0) throw new Error('Invalid distance');

        const body = { start_point: { lat, lng }, distance_km: dist, mode };
        const prefs = {};
        if (selectedCats.size > 0 && selectedCats.size < Object.keys(CATEGORY_INFO).length)
            prefs.poi_categories = [...selectedCats];
        if (document.getElementById('hiddenGems').checked) prefs.hidden_gems = true;
        if (Object.keys(prefs).length) body.preferences = prefs;

        const endpoint = API_BASE + '/api/v1/routes/loop';
        console.log('API call:', endpoint, body);
        const resp = await fetch(endpoint, {
            method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body)
        });
        if (!resp.ok) throw new Error('API error (' + resp.status + '): ' + await resp.text());

        const data = await resp.json();
        currentRoutes = data.routes;
        if (!currentRoutes?.length) throw new Error('No routes returned');
        displayRouteInfo(); displayRoute(0);
        collapsePanel();
    } catch (e) {
        console.error(e);
        err.className = 'error'; err.textContent = e.message; err.style.display = 'block';
    } finally {
        btn.disabled = false; btn.textContent = 'Generate Route'; panel.classList.remove('loading');
    }
}

function visualizeRoutes() {
    try {
        const data = JSON.parse(document.getElementById('apiResponse').value);
        currentRoutes = data.routes;
        if (!currentRoutes?.length) { alert('No routes in response'); return; }
        displayRouteInfo(); displayRoute(0);
    } catch (e) { alert('Invalid JSON: ' + e.message); }
}

// ── Route info panel ───────────────────────────────────────
function displayRouteInfo() {
    const div = document.getElementById('routeInfo');
    div.style.display = 'block';
    let h = '<h4>Routes</h4>';
    currentRoutes.forEach((r, i) => {
        h += '<div class="route-card ' + (i === selectedRouteIndex ? 'selected' : '') + '" onclick="displayRoute(' + i + ')">'
            + '<div class="rc-title">Route ' + (i + 1) + '</div>'
            + '<div class="rc-meta">' + r.distance_km.toFixed(1) + ' km &middot; '
            + r.estimated_duration_minutes + ' min &middot; Score ' + r.score.toFixed(1) + '/10<br>'
            + r.pois.length + ' waypoints'
            + (r.snapped_pois ? ' &middot; ' + r.snapped_pois.length + ' nearby' : '') + '</div></div>';
    });
    h += '<div class="export-row">'
        + '<button class="btn-export" onclick="exportGPX()">GPX</button>'
        + '<button class="btn-export" onclick="exportGoogleMaps()">Google Maps</button>'
        + '<button class="btn-export" onclick="exportGeoJSON()">GeoJSON</button>'
        + '</div>';
    div.innerHTML = h;
}

function renderMetrics(m) {
    if (!m) return '';
    return '<dl class="metrics-grid">'
        + '<dt>Circularity</dt><dd>' + (m.circularity * 100).toFixed(0) + '%</dd>'
        + '<dt>Convexity</dt><dd>' + (m.convexity * 100).toFixed(0) + '%</dd>'
        + '<dt>Overlap</dt><dd>' + (m.path_overlap_pct * 100).toFixed(0) + '%</dd>'
        + '<dt>POI density</dt><dd>' + m.poi_density_per_km.toFixed(1) + '/km</dd>'
        + '<dt>Entropy</dt><dd>' + m.category_entropy.toFixed(2) + '</dd>'
        + '<dt>Landmark</dt><dd>' + (m.landmark_coverage * 100).toFixed(0) + '%</dd>'
        + '<dt>Density</dt><dd>' + m.poi_density_context + '</dd></dl>';
}

// ── Display route on map ───────────────────────────────────
function displayRoute(idx) {
    selectedRouteIndex = idx;
    const route = currentRoutes[idx];
    clearMap(); hiddenCategories.clear();

    const coords = route.path.map(c => [c.lng, c.lat]);
    if (map.getSource('route')) {
        map.getSource('route').setData({ type: 'Feature', geometry: { type: 'LineString', coordinates: coords } });
    } else {
        map.addSource('route', { type: 'geojson', data: { type: 'Feature', geometry: { type: 'LineString', coordinates: coords } } });
        map.addLayer({ id: 'route', type: 'line', source: 'route',
            layout: { 'line-join': 'round', 'line-cap': 'round' },
            paint: { 'line-color': '#4264fb', 'line-width': 4 } });
    }

    // Start marker
    const s = route.path[0];
    const sm = new mapboxgl.Marker(mkEl('start-marker'))
        .setLngLat([s.lng, s.lat]).setPopup(new mapboxgl.Popup().setHTML('<strong>Start/End</strong>')).addTo(map);
    currentMarkers.push({ marker: sm, category: null, type: 'start' });

    // Waypoint POIs
    route.pois.forEach(poi => {
        const cat = poi.category, color = (CATEGORY_INFO[cat] || {}).color || '#4264fb';
        const el = mkEl('poi-marker', color, poi.order_in_route); el.dataset.category = cat;
        const popup = '<strong>' + poi.name + '</strong> (Waypoint)<br>Category: ' + cat
            + '<br>Popularity: ' + poi.popularity_score.toFixed(0) + '/100'
            + (poi.description ? '<br>' + poi.description : '')
            + '<br>Distance: ' + poi.distance_from_start_km.toFixed(1) + ' km';
        const m = new mapboxgl.Marker(el).setLngLat([poi.coordinates.lng, poi.coordinates.lat])
            .setPopup(new mapboxgl.Popup().setHTML(popup)).addTo(map);
        currentMarkers.push({ marker: m, category: cat, type: 'waypoint' });
    });

    // Snapped POIs
    if (route.snapped_pois) {
        route.snapped_pois.forEach(poi => {
            const cat = poi.category, color = (CATEGORY_INFO[cat] || {}).color || '#ff9500';
            const el = mkEl('snapped-poi-marker', color); el.dataset.category = cat;
            const popup = '<strong>' + poi.name + '</strong> (Nearby)<br>Category: ' + cat
                + '<br>Popularity: ' + poi.popularity_score.toFixed(0) + '/100'
                + (poi.description ? '<br>' + poi.description : '')
                + '<br>From path: ' + poi.distance_from_path_m.toFixed(0) + 'm'
                + ' &middot; Along: ' + poi.distance_from_start_km.toFixed(1) + ' km';
            const m = new mapboxgl.Marker(el).setLngLat([poi.coordinates.lng, poi.coordinates.lat])
                .setPopup(new mapboxgl.Popup().setHTML(popup)).addTo(map);
            currentMarkers.push({ marker: m, category: cat, type: 'snapped' });
        });
    }

    // Fit bounds, suppress sync during animation
    const bounds = new mapboxgl.LngLatBounds();
    coords.forEach(c => bounds.extend(c));
    _suppressSync = true;
    map.fitBounds(bounds, { padding: 50 });
    map.once('moveend', () => { _suppressSync = false; });

    displayRouteInfo(); buildPoiFilter();
    const cards = document.querySelectorAll('.route-card');
    if (cards[idx]) cards[idx].insertAdjacentHTML('beforeend', renderMetrics(route.metrics));
    collapsePanel();
}

// ── POI filter ─────────────────────────────────────────────
function buildPoiFilter() {
    const div = document.getElementById('poiFilter');
    const cats = new Set();
    currentMarkers.forEach(m => { if (m.category) cats.add(m.category); });
    if (!cats.size) { div.style.display = 'none'; return; }

    div.style.display = 'block';
    let h = '<div class="filter-header"><span>Filter POIs</span><span class="filter-links">'
        + '<a onclick="setAllFilters(true)">All</a><a onclick="setAllFilters(false)">None</a></span></div>';
    for (const cat of [...cats].sort()) {
        const info = CATEGORY_INFO[cat] || { label: cat, color: '#888' };
        h += '<button class="filter-btn active" style="background:' + info.color
            + ';border-color:transparent;color:white" onclick="toggleCategoryFilter(\'' + cat + '\',this)">'
            + info.label + '</button>';
    }
    div.innerHTML = h;
}

function toggleCategoryFilter(cat, btn) {
    const info = CATEGORY_INFO[cat] || { color: '#888' };
    if (hiddenCategories.has(cat)) {
        hiddenCategories.delete(cat);
        btn.className = 'filter-btn active';
        btn.style.background = info.color; btn.style.borderColor = 'transparent'; btn.style.color = 'white';
    } else {
        hiddenCategories.add(cat);
        btn.className = 'filter-btn inactive';
        btn.style.background = 'white'; btn.style.borderColor = '#ccc'; btn.style.color = '#333';
    }
    applyMarkerVis();
}

function setAllFilters(show) {
    hiddenCategories.clear();
    if (!show) currentMarkers.forEach(m => { if (m.category) hiddenCategories.add(m.category); });
    document.querySelectorAll('#poiFilter .filter-btn').forEach(btn => {
        const label = btn.textContent.trim();
        for (const [k, info] of Object.entries(CATEGORY_INFO)) {
            if (info.label === label) {
                if (show) { btn.className = 'filter-btn active'; btn.style.background = info.color; btn.style.borderColor = 'transparent'; btn.style.color = 'white'; }
                else { btn.className = 'filter-btn inactive'; btn.style.background = 'white'; btn.style.borderColor = '#ccc'; btn.style.color = '#333'; }
                break;
            }
        }
    });
    applyMarkerVis();
}

function applyMarkerVis() {
    currentMarkers.forEach(m => {
        if (!m.category) return;
        m.marker.getElement().style.display = hiddenCategories.has(m.category) ? 'none' : '';
    });
}

function clearMap() { currentMarkers.forEach(m => m.marker.remove()); currentMarkers = []; }

// ── Export helpers ─────────────────────────────────────────
function escapeXML(s) {
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;').replace(/'/g, '&apos;');
}

function downloadFile(content, filename, mimeType) {
    const blob = new Blob([content], { type: mimeType });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = filename;
    document.body.appendChild(a); a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

function exportGPX() {
    const route = currentRoutes[selectedRouteIndex];
    if (!route) return;
    const name = 'EasyRoute Loop — ' + route.distance_km.toFixed(1) + ' km';
    let gpx = '<?xml version="1.0" encoding="UTF-8"?>\n'
        + '<gpx version="1.1" creator="EasyRoute"\n'
        + '  xmlns="http://www.topografix.com/GPX/1/1"\n'
        + '  xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"\n'
        + '  xsi:schemaLocation="http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd">\n'
        + '  <metadata><name>' + escapeXML(name) + '</name></metadata>\n';

    // Waypoint POIs
    route.pois.forEach(poi => {
        gpx += '  <wpt lat="' + poi.coordinates.lat + '" lon="' + poi.coordinates.lng + '">\n'
            + '    <name>' + escapeXML(poi.name) + '</name>\n'
            + '    <desc>' + escapeXML('Waypoint #' + poi.order_in_route + ' — ' + poi.category) + '</desc>\n'
            + '  </wpt>\n';
    });

    // Snapped POIs
    (route.snapped_pois || []).forEach(poi => {
        gpx += '  <wpt lat="' + poi.coordinates.lat + '" lon="' + poi.coordinates.lng + '">\n'
            + '    <name>' + escapeXML(poi.name) + '</name>\n'
            + '    <desc>' + escapeXML('Nearby — ' + poi.category) + '</desc>\n'
            + '  </wpt>\n';
    });

    // Track
    gpx += '  <trk><name>' + escapeXML(name) + '</name><trkseg>\n';
    route.path.forEach(p => {
        gpx += '    <trkpt lat="' + p.lat + '" lon="' + p.lng + '"></trkpt>\n';
    });
    gpx += '  </trkseg></trk>\n</gpx>\n';

    downloadFile(gpx, 'easyroute-' + route.distance_km.toFixed(1) + 'km.gpx', 'application/gpx+xml');
}

function exportGoogleMaps() {
    const route = currentRoutes[selectedRouteIndex];
    if (!route) return;
    const start = route.path[0];
    const origin = start.lat + ',' + start.lng;
    const mode = document.getElementById('mode').value;
    const travelMode = mode === 'bike' ? 'bicycling' : 'walking';

    const waypoints = route.pois
        .slice()
        .sort((a, b) => a.order_in_route - b.order_in_route)
        .slice(0, 8)
        .map(p => p.coordinates.lat + ',' + p.coordinates.lng)
        .join('|');

    let url = 'https://www.google.com/maps/dir/?api=1'
        + '&origin=' + encodeURIComponent(origin)
        + '&destination=' + encodeURIComponent(origin)
        + '&travelmode=' + travelMode;
    if (waypoints) url += '&waypoints=' + encodeURIComponent(waypoints);

    window.open(url, '_blank');
}

function exportGeoJSON() {
    const route = currentRoutes[selectedRouteIndex];
    if (!route) return;
    const features = [];

    // Route line
    features.push({
        type: 'Feature',
        geometry: { type: 'LineString', coordinates: route.path.map(p => [p.lng, p.lat]) },
        properties: {
            type: 'route', distance_km: route.distance_km,
            duration_minutes: route.estimated_duration_minutes, score: route.score
        }
    });

    // Waypoint POIs
    route.pois.forEach(poi => {
        features.push({
            type: 'Feature',
            geometry: { type: 'Point', coordinates: [poi.coordinates.lng, poi.coordinates.lat] },
            properties: {
                type: 'waypoint', name: poi.name, category: poi.category,
                order: poi.order_in_route, popularity: poi.popularity_score
            }
        });
    });

    // Snapped POIs
    (route.snapped_pois || []).forEach(poi => {
        features.push({
            type: 'Feature',
            geometry: { type: 'Point', coordinates: [poi.coordinates.lng, poi.coordinates.lat] },
            properties: {
                type: 'nearby', name: poi.name, category: poi.category,
                distance_from_path_m: poi.distance_from_path_m
            }
        });
    });

    const geojson = JSON.stringify({ type: 'FeatureCollection', features }, null, 2);
    downloadFile(geojson, 'easyroute-' + route.distance_km.toFixed(1) + 'km.geojson', 'application/geo+json');
}

// ── Locate on map ───────────────────────────────────────────
function locateOnMap() {
    if (!navigator.geolocation) return;
    const btn = document.getElementById('locateBtn');
    btn.classList.add('locating');
    navigator.geolocation.getCurrentPosition(
        pos => {
            btn.classList.remove('locating');
            const { latitude: lat, longitude: lng } = pos.coords;
            document.getElementById('startLat').value = lat.toFixed(4);
            document.getElementById('startLng').value = lng.toFixed(4);
            _suppressSync = true;
            map.flyTo({ center: [lng, lat], zoom: 14 });
            map.once('moveend', () => { _suppressSync = false; });
        },
        () => { btn.classList.remove('locating'); },
        { enableHighAccuracy: true, timeout: 10000 }
    );
}

// ── Mobile bottom sheet ─────────────────────────────────────
function collapsePanel() {
    document.querySelector('.panel').classList.add('collapsed');
}

function expandPanel() {
    document.querySelector('.panel').classList.remove('collapsed');
}

function initMobileSheet() {
    if (window.innerWidth > 600) return;

    const panel = document.querySelector('.panel');
    const handle = document.querySelector('.panel-handle');
    let startY = 0, startTime = 0, currentY = 0, dragging = false;

    handle.addEventListener('touchstart', e => {
        startY = e.touches[0].clientY;
        startTime = Date.now();
        currentY = 0;
        dragging = true;
        panel.classList.add('dragging');
    }, { passive: true });

    handle.addEventListener('touchmove', e => {
        if (!dragging) return;
        e.preventDefault(); // stop browser scroll during drag
        currentY = e.touches[0].clientY - startY;
        const isCollapsed = panel.classList.contains('collapsed');
        if (isCollapsed) {
            const offset = Math.min(0, currentY);
            panel.style.transform = 'translateY(calc(100% - 64px + ' + offset + 'px))';
        } else {
            const offset = Math.max(0, currentY); // clamp: can't drag above expanded
            panel.style.transform = 'translateY(' + offset + 'px)';
        }
    }, { passive: false }); // non-passive so preventDefault() works

    handle.addEventListener('touchend', () => {
        if (!dragging) return;
        dragging = false;
        panel.classList.remove('dragging');
        panel.style.transform = '';

        const elapsed = Date.now() - startTime;
        const velocity = elapsed > 0 ? Math.abs(currentY) / elapsed : 0;
        const isCollapsed = panel.classList.contains('collapsed');

        if (isCollapsed) {
            if (currentY < -30 || (velocity > 0.5 && currentY < 0))
                panel.classList.remove('collapsed');
        } else {
            if (currentY > 30 || (velocity > 0.5 && currentY > 0))
                panel.classList.add('collapsed');
        }
    });

    // Start collapsed
    panel.classList.add('collapsed');
}

// ── Init ───────────────────────────────────────────────────
buildCatPills();
updateMapCenter();
initMobileSheet();
