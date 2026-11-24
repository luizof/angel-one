<script lang="ts">
  // Escolher coordenadas: arraste o mapa até o pino ou use "estou aqui agora".
  import L from 'leaflet'
  import 'leaflet/dist/leaflet.css'
  import { onMount } from 'svelte'
  import { app } from '../lib/store.svelte'

  let { lat = $bindable(null as number | null), lon = $bindable(null as number | null) }: { lat?: number | null; lon?: number | null } = $props()

  let el: HTMLDivElement
  let map: L.Map | undefined
  let moving = $state(false)
  let locating = $state(false)
  let sat = $state(true)
  let base: L.TileLayer

  const url = (s: boolean) =>
    s
      ? 'https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}'
      : 'https://{s}.basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}{r}.png'

  onMount(() => {
    const start: [number, number] =
      lat != null && lon != null ? [lat, lon] : app.origin ? [app.origin.lat, app.origin.lon] : [-27.435, -48.5]
    map = L.map(el, { zoomControl: false, attributionControl: true }).setView(start, lat != null ? 15 : 13)
    base = L.tileLayer(url(sat), { maxZoom: 19, subdomains: 'abcd', attribution: '© OpenStreetMap · CARTO · Esri' }).addTo(map)
    const sync = () => {
      const c = map!.getCenter()
      lat = Number(c.lat.toFixed(6))
      lon = Number(c.lng.toFixed(6))
    }
    map.on('movestart', () => (moving = true))
    map.on('click', (e: L.LeafletMouseEvent) => map?.panTo(e.latlng))
    map.on('moveend', () => {
      moving = false
      sync()
    })
    if (lat == null) sync()
    setTimeout(() => map?.invalidateSize(), 350)
    return () => map?.remove()
  })

  async function here() {
    locating = true
    const p = await app.sampleGeo()
    locating = false
    if (p && map) map.flyTo([p.lat, p.lon], 16, { duration: 0.8 })
    else app.toast('Não consegui pegar sua localização. Confira a permissão do navegador.', 'err', '📍')
  }
  function toggle() {
    sat = !sat
    base?.setUrl(url(sat))
  }
</script>

<div class="picker">
  <div class="map" bind:this={el}></div>
  <div class="pin" class:moving aria-hidden="true">
    <svg width="40" height="52" viewBox="0 0 40 52"><path d="M20 50s16-15 16-29A16 16 0 0 0 4 21c0 14 16 29 16 29z" fill="var(--accent)" stroke="#fff" stroke-width="3" /><circle cx="20" cy="20" r="6" fill="#fff" /></svg>
    <span class="shadow"></span>
  </div>
  <div class="bar">
    <button class="btn sm gold" onclick={here} disabled={locating}>
      {locating ? 'Localizando…' : '📍 Estou aqui agora'}
    </button>
    <button class="btn sm ghost" onclick={toggle}>{sat ? 'Mapa' : 'Satélite'}</button>
  </div>
  {#if lat != null && lon != null}
    <div class="coords">{lat.toFixed(5)}, {lon.toFixed(5)}</div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    height: 320px;
    border-radius: 22px;
    overflow: hidden;
    border: 1px solid var(--line);
    isolation: isolate;
  }
  .map {
    position: absolute;
    inset: 0;
  }
  .pin {
    position: absolute;
    left: 50%;
    top: 50%;
    z-index: 600;
    transform: translate(-50%, -100%);
    pointer-events: none;
    transition: transform 0.3s var(--spring);
  }
  .pin svg {
    display: block;
    filter: drop-shadow(0 6px 8px rgba(0, 0, 0, 0.35));
  }
  .pin.moving {
    transform: translate(-50%, calc(-100% - 12px));
  }
  .shadow {
    position: absolute;
    left: 50%;
    bottom: -4px;
    width: 14px;
    height: 5px;
    margin-left: -7px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.35);
  }
  .bar {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 10px;
    z-index: 600;
    display: flex;
    gap: 8px;
    justify-content: space-between;
  }
  .coords {
    position: absolute;
    top: 10px;
    left: 10px;
    z-index: 600;
    padding: 4px 10px;
    border-radius: 10px;
    font: 800 12px var(--font-body);
    background: rgba(255, 255, 255, 0.85);
    color: #0c2a37;
  }
</style>
