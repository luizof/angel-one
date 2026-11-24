<script lang="ts">
  // Mapa (Leaflet) com destinos, barco, minha posição e rota pela água.
  import L from 'leaflet'
  import 'leaflet/dist/leaflet.css'
  import { onMount } from 'svelte'
  import { app } from '../lib/store.svelte'
  import type { Destination } from '../lib/types'

  let {
    destinations = [] as Destination[],
    selected = null as number | null,
    onselect,
    height = '42vh',
    interactive = true,
  }: {
    destinations?: Destination[]
    selected?: number | null
    onselect?: (id: number) => void
    height?: string
    interactive?: boolean
  } = $props()

  let el: HTMLDivElement
  let map: L.Map | undefined = $state()
  let layer: L.LayerGroup
  let routeLayer: L.LayerGroup
  let boatLayer: L.LayerGroup
  let base: L.TileLayer
  let sat = $state(localStorage.getItem('ao.mapSat') === '1')
  let fitted = false

  const isNight = $derived(app.activeBand === 'noite')

  function tileUrl() {
    if (sat) return 'https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}'
    return isNight
      ? 'https://{s}.basemaps.cartocdn.com/rastertiles/dark_all/{z}/{x}/{y}{r}.png'
      : 'https://{s}.basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}{r}.png'
  }
  const ATTR = '© colaboradores do OpenStreetMap · © CARTO · Esri'

  onMount(() => {
    map = L.map(el, {
      zoomControl: false,
      attributionControl: true,
      dragging: interactive,
      touchZoom: interactive,
      scrollWheelZoom: false,
      doubleClickZoom: interactive,
      boxZoom: false,
      keyboard: false,
      tapTolerance: 20,
    }).setView([-27.43, -48.49], 12)
    base = L.tileLayer(tileUrl(), { maxZoom: 18, attribution: ATTR, subdomains: 'abcd', detectRetina: true }).addTo(map)
    routeLayer = L.layerGroup().addTo(map)
    layer = L.layerGroup().addTo(map)
    boatLayer = L.layerGroup().addTo(map)
    setTimeout(() => map?.invalidateSize(), 300)
    return () => map?.remove()
  })

  $effect(() => {
    const u = tileUrl()
    if (base) base.setUrl(u)
  })

  function esc(s: string) {
    return s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!)
  }

  // marcadores dos destinos
  $effect(() => {
    if (!map || !layer) return
    const sel = selected
    layer.clearLayers()
    for (const d of destinations) {
      const thumb = d.photo && /^\/media\/[a-zA-Z0-9_-]+\.(jpg|png|webp)$/.test(d.photo) ? d.photo : '/icons/icon-192.png'
      const html = `<div class="dm ${sel === d.id ? 'sel' : ''}">
          <div class="ph" style="background-image:url('${thumb}')"></div>
          ${d.votes ? `<span class="v">${d.votes}</span>` : ''}
          <span class="nm">${esc(d.name)}</span>
        </div>`
      const icon = L.divIcon({ html, className: 'dm-wrap', iconSize: [46, 46], iconAnchor: [23, 23] })
      const m = L.marker([d.lat, d.lon], { icon, zIndexOffset: sel === d.id ? 1000 : 0 })
      m.on('click', () => onselect?.(d.id))
      layer.addLayer(m)
    }
    if (!fitted && destinations.length) {
      fitted = true
      const pts: L.LatLngExpression[] = destinations.map((d) => [d.lat, d.lon])
      const o = app.origin
      if (o) pts.push([o.lat, o.lon])
      map.fitBounds(L.latLngBounds(pts), { padding: [40, 40], maxZoom: 14 })
    }
  })

  // barco + eu
  $effect(() => {
    if (!map || !boatLayer) return
    boatLayer.clearLayers()
    const s = app.status
    if (s && s.lat != null && s.lon != null) {
      const rot = s.heading ?? 0
      const html = `<div class="boat ${s.moving ? 'mv' : ''}"><span class="pulse"></span><svg viewBox="0 0 24 24" width="30" height="30" style="transform:rotate(${rot}deg)"><path d="M12 2c3 3 4.5 7 4.5 11.5V21h-9v-7.5C7.5 9 9 5 12 2z" fill="white" stroke="#0c2a37" stroke-width="1.4"/><path d="M9.5 13h5v4h-5z" fill="#0c2a37" opacity=".6"/></svg></div>`
      boatLayer.addLayer(L.marker([s.lat, s.lon], { icon: L.divIcon({ html, className: 'boat-wrap', iconSize: [30, 30], iconAnchor: [15, 15] }), zIndexOffset: 2000 }))
    }
    const me = app.myPos
    if (me) {
      boatLayer.addLayer(L.circleMarker([me.lat, me.lon], { radius: 7, color: '#fff', weight: 3, fillColor: '#2b8cff', fillOpacity: 1 }))
    }
  })

  // rota até o destino selecionado
  $effect(() => {
    if (!map || !routeLayer) return
    routeLayer.clearLayers()
    if (selected == null) return
    const r = app.routes[selected]
    const d = destinations.find((x) => x.id === selected)
    if (!d) return
    if (r && r.path?.length > 1) {
      const color = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || '#00a6b4'
      L.polyline(r.path, { color: '#fff', weight: 8, opacity: 0.7, lineCap: 'round' }).addTo(routeLayer)
      L.polyline(r.path, { color, weight: 4.5, dashArray: '2 10', lineCap: 'round', className: 'route-ants' }).addTo(routeLayer)
      map.flyToBounds(L.latLngBounds(r.path), { padding: [50, 50], maxZoom: 15, duration: 0.8 })
    } else {
      map.flyTo([d.lat, d.lon], 14, { duration: 0.8 })
    }
  })

  function toggleSat() {
    sat = !sat
    localStorage.setItem('ao.mapSat', sat ? '1' : '0')
  }
  function recenter() {
    const o = app.origin
    if (o && map) map.flyTo([o.lat, o.lon], 13, { duration: 0.8 })
    else app.startGeo()
  }
</script>

<div class="map-wrap" style="height:{height}">
  <div class="map" bind:this={el}></div>
  {#if interactive}
    <div class="ctrls">
      <button class="icon-btn" onclick={toggleSat} aria-label="Satélite">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l9 5-9 5-9-5z" /><path d="M3 13l9 5 9-5" /></svg>
      </button>
      <button class="icon-btn" onclick={recenter} aria-label="Centralizar">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="3.2" /><circle cx="12" cy="12" r="7.5" /><path d="M12 2v2.5M12 19.5V22M2 12h2.5M19.5 12H22" /></svg>
      </button>
    </div>
  {/if}
</div>

<style>
  .map-wrap {
    position: relative;
    border-radius: var(--r-lg);
    overflow: hidden;
    border: 1px solid var(--line);
    box-shadow: var(--shadow);
    isolation: isolate;
  }
  .map {
    position: absolute;
    inset: 0;
  }
  .ctrls {
    position: absolute;
    right: 10px;
    top: 10px;
    display: grid;
    gap: 8px;
    z-index: 500;
  }
  .ctrls .icon-btn {
    background: color-mix(in srgb, var(--card-solid) 85%, transparent);
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
  }
  :global(.dm-wrap) {
    background: none;
    border: 0;
  }
  :global(.dm) {
    position: relative;
    width: 46px;
    height: 46px;
    transition: transform 0.4s var(--spring);
  }
  :global(.dm .ph) {
    width: 46px;
    height: 46px;
    border-radius: 50%;
    background-size: cover;
    background-position: center;
    border: 3px solid #fff;
    box-shadow: 0 6px 16px -4px rgba(0, 0, 0, 0.5);
  }
  :global(.dm.sel) {
    transform: scale(1.3) translateY(-6px);
    z-index: 10;
  }
  :global(.dm.sel .ph) {
    border-color: var(--accent);
  }
  :global(.dm .v) {
    position: absolute;
    top: -6px;
    right: -8px;
    min-width: 20px;
    height: 20px;
    padding: 0 5px;
    border-radius: 99px;
    background: var(--gold);
    color: #3b2400;
    font: 900 11px/20px var(--font-body);
    text-align: center;
    box-shadow: 0 0 0 2px #fff;
  }
  :global(.dm .nm) {
    position: absolute;
    top: 50px;
    left: 50%;
    transform: translateX(-50%);
    white-space: nowrap;
    font: 800 11px var(--font-body);
    color: #0c2a37;
    background: rgba(255, 255, 255, 0.9);
    padding: 1px 7px;
    border-radius: 8px;
  }
  :global(.boat-wrap) {
    background: none;
    border: 0;
  }
  :global(.boat) {
    position: relative;
    width: 30px;
    height: 30px;
    filter: drop-shadow(0 3px 6px rgba(0, 0, 0, 0.45));
  }
  :global(.boat svg) {
    position: relative;
    z-index: 1;
    transition: transform 1s;
  }
  :global(.boat .pulse) {
    position: absolute;
    inset: -12px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    animation: boatpulse 2.4s ease-out infinite;
  }
  @keyframes -global-boatpulse {
    from {
      transform: scale(0.4);
      opacity: 1;
    }
    to {
      transform: scale(1.6);
      opacity: 0;
    }
  }
  :global(.route-ants) {
    animation: ants 1.2s linear infinite;
  }
  @keyframes -global-ants {
    to {
      stroke-dashoffset: -24;
    }
  }
</style>
