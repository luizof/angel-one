<script lang="ts">
  import { onMount } from 'svelte'
  import { prepareImage } from '../lib/api'
  import Sheet from './Sheet.svelte'

  let {
    file, aspect = '1 / 1', ondone, oncancel,
  }: {
    file: File
    aspect?: string
    ondone: (blob: Blob) => void
    oncancel: () => void
  } = $props()

  let url = $state('')
  let src: Blob | null = null
  let image = $state<HTMLImageElement>()
  let viewport: HTMLDivElement
  let ready = $state(false)
  let w = $state(1)
  let h = $state(1)
  let vw = $state(1)
  let vh = $state(1)
  let zoom = $state(1)
  let dx = $state(0)
  let dy = $state(0)
  let startX = 0
  let startY = 0
  let startDx = 0
  let startDy = 0
  let working = $state(false)
  let open = $state(true)
  const ratio = $derived((() => {
    const n = aspect.split('/').map(Number)
    return n.length === 2 && n.every(x => x > 0 && Number.isFinite(x)) ? n[0] / n[1] : 1
  })())
  const base = $derived(Math.max(vw / w, vh / h))
  const scale = $derived(base * zoom)
  const sw = $derived(w * scale)
  const sh = $derived(h * scale)

  onMount(() => {
    let cancelled = false
    let observer: ResizeObserver | undefined
    prepareImage(file).then(b => {
      if (cancelled) return
      src = b
      url = URL.createObjectURL(b)
    }).catch(() => oncancel())
    observer = new ResizeObserver(() => {
      if (!viewport) return
      vw = viewport.clientWidth
      vh = viewport.clientHeight
      constrain()
    })
    if (viewport) observer.observe(viewport)
    return () => {
      cancelled = true
      observer?.disconnect()
      if (url) URL.revokeObjectURL(url)
    }
  })

  function loaded() {
    if (!image) return
    w = image.naturalWidth
    h = image.naturalHeight
    ready = true
    if (viewport) {
      vw = viewport.clientWidth
      vh = viewport.clientHeight
    }
    constrain()
  }
  function constrain() {
    dx = Math.max((vw - sw) / 2, Math.min((sw - vw) / 2, dx))
    dy = Math.max((vh - sh) / 2, Math.min((sh - vh) / 2, dy))
  }
  function down(e: PointerEvent) {
    e.preventDefault()
    viewport.setPointerCapture(e.pointerId)
    startX = e.clientX
    startY = e.clientY
    startDx = dx
    startDy = dy
  }
  function move(e: PointerEvent) {
    if (!viewport?.hasPointerCapture(e.pointerId)) return
    dx = startDx + e.clientX - startX
    dy = startDy + e.clientY - startY
    constrain()
  }
  function onzoom() {
    constrain()
  }
  function close() {
    open = false
    oncancel()
  }
  function output(crop: boolean) {
    if (!src || !image || !ready || working) return
    if (!crop) {
      ondone(src)
      return
    }
    working = true
    const canvas = document.createElement('canvas')
    canvas.width = Math.min(1600, Math.round(vw / scale))
    canvas.height = Math.round(canvas.width / ratio)
    const x = (w - vw / scale) / 2 - dx / scale
    const y = (h - vh / scale) / 2 - dy / scale
    canvas.getContext('2d')!.drawImage(image, x, y, vw / scale, vh / scale, 0, 0, canvas.width, canvas.height)
    canvas.toBlob(blob => {
      working = false
      if (blob) ondone(blob)
      else oncancel()
    }, 'image/jpeg', 0.9)
  }
</script>

<Sheet bind:open title="Recortar foto" onclose={oncancel}>
  <p class="small muted">Arraste para enquadrar; use o controle para aproximar.</p>
  <div class="viewport" bind:this={viewport} style:aspect-ratio={ratio} onpointerdown={down} onpointermove={move} role="presentation">
    {#if url}
      <img bind:this={image} src={url} alt="Prévia da foto a recortar" onload={loaded}
        style:width="{sw}px" style:height="{sh}px"
        style:transform="translate(calc(-50% + {dx}px), calc(-50% + {dy}px))" draggable="false" />
    {/if}
    {#if !ready}<span class="loading">Preparando foto…</span>{/if}
    <div class="grid" aria-hidden="true"><i></i><i></i><i></i><i></i></div>
  </div>
  <label class="zoom"><span>−</span><input type="range" min="1" max="3" step=".01" bind:value={zoom} oninput={onzoom} aria-label="Aproximar foto" /><span>＋</span></label>
  {#snippet footer()}
    <div class="buttons">
      <button class="btn ghost" onclick={() => output(false)} disabled={!ready || working}>Imagem inteira</button>
      <button class="btn grow" onclick={() => output(true)} disabled={!ready || working}>{working ? 'Recortando…' : 'Usar recorte'}</button>
    </div>
  {/snippet}
</Sheet>

<style>
  .viewport { position:relative; width:100%; margin:14px auto 0; overflow:hidden; border-radius:20px; touch-action:none; background:var(--card-2); }
  img { position:absolute; top:50%; left:50%; max-width:none; object-fit:fill; pointer-events:none; user-select:none; }
  .grid { position:absolute; inset:0; border:2px solid rgba(255,255,255,.75); pointer-events:none; display:grid; grid-template-columns:repeat(3,1fr); grid-template-rows:repeat(3,1fr); }
  .grid i { border-right:1px solid rgba(255,255,255,.42); border-bottom:1px solid rgba(255,255,255,.42); }
  .grid i:nth-child(3n) { border-right:0; }
  .grid i:nth-child(n+7) { border-bottom:0; }
  .loading { position:absolute; inset:0; display:grid; place-items:center; color:var(--muted); }
  .zoom { display:flex; gap:15px; align-items:center; margin:18px 4px 8px; color:var(--accent); font-size:23px; }
  .zoom input { flex:1; accent-color:var(--accent); }
  .buttons { display:flex; gap:8px; }
</style>
