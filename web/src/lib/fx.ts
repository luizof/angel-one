// Partículas leves em canvas: penas, confete e bolhas.
type Kind = 'feathers' | 'confetti' | 'bubbles' | 'sparkles'

interface P {
  x: number
  y: number
  vx: number
  vy: number
  r: number
  vr: number
  s: number
  life: number
  max: number
  color: string
  kind: Kind
  sway: number
  phase: number
}

let canvas: HTMLCanvasElement | null = null
let ctx: CanvasRenderingContext2D | null = null
let parts: P[] = []
let raf = 0
let last = 0

function ensure() {
  if (canvas) return
  canvas = document.createElement('canvas')
  canvas.className = 'fx-canvas'
  Object.assign(canvas.style, {
    position: 'fixed', inset: '0', width: '100vw', height: '100vh', pointerEvents: 'none', zIndex: '9999',
  })
  document.body.appendChild(canvas)
  ctx = canvas.getContext('2d')
  resize()
  window.addEventListener('resize', resize)
}

function resize() {
  if (!canvas) return
  const dpr = Math.min(2, window.devicePixelRatio || 1)
  canvas.width = innerWidth * dpr
  canvas.height = innerHeight * dpr
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0)
}

function cssVar(name: string, fallback: string) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback
}

function drawFeather(c: CanvasRenderingContext2D, p: P) {
  c.save()
  c.translate(p.x, p.y)
  c.rotate(p.r)
  c.scale(p.s, p.s)
  c.beginPath()
  c.moveTo(0, -14)
  c.bezierCurveTo(7, -9, 7, 6, 0, 14)
  c.bezierCurveTo(-7, 6, -7, -9, 0, -14)
  c.fillStyle = 'rgba(255,255,255,0.95)'
  c.shadowColor = 'rgba(255, 210, 120, 0.8)'
  c.shadowBlur = 6
  c.fill()
  c.shadowBlur = 0
  c.strokeStyle = 'rgba(214,170,80,0.9)'
  c.lineWidth = 1
  c.beginPath()
  c.moveTo(0, -12)
  c.lineTo(0, 17)
  c.stroke()
  for (let i = -8; i < 10; i += 4) {
    c.beginPath()
    c.moveTo(0, i)
    c.lineTo(4, i - 3)
    c.moveTo(0, i)
    c.lineTo(-4, i - 3)
    c.strokeStyle = 'rgba(214,170,80,0.35)'
    c.stroke()
  }
  c.restore()
}

function tick(t: number) {
  if (!ctx || !canvas) return
  const dt = Math.min(0.05, (t - last) / 1000 || 0.016)
  last = t
  ctx.clearRect(0, 0, innerWidth, innerHeight)
  parts = parts.filter((p) => p.life < p.max)
  for (const p of parts) {
    p.life += dt
    const k = p.life / p.max
    if (p.kind === 'feathers') {
      p.vy = Math.min(p.vy + 90 * dt, 70)
      p.vx *= 0.97
      p.x += (p.vx + Math.sin(p.life * 3 + p.phase) * p.sway) * dt
      p.y += p.vy * dt
      p.r = Math.sin(p.life * 2.5 + p.phase) * 0.9
    } else if (p.kind === 'confetti') {
      p.vy += 520 * dt
      p.vx *= 0.985
      p.x += p.vx * dt
      p.y += p.vy * dt
      p.r += p.vr * dt
    } else if (p.kind === 'bubbles') {
      p.vy -= 30 * dt
      p.x += Math.sin(p.life * 4 + p.phase) * 18 * dt
      p.y += p.vy * dt
    } else {
      p.x += p.vx * dt
      p.y += p.vy * dt
      p.vx *= 0.92
      p.vy *= 0.92
    }
    ctx.globalAlpha = Math.max(0, 1 - Math.max(0, k - 0.7) / 0.3)
    if (p.kind === 'feathers') drawFeather(ctx, p)
    else if (p.kind === 'confetti') {
      ctx.save()
      ctx.translate(p.x, p.y)
      ctx.rotate(p.r)
      ctx.scale(1, Math.cos(p.life * 9 + p.phase))
      ctx.fillStyle = p.color
      ctx.fillRect(-4 * p.s, -2.5 * p.s, 8 * p.s, 5 * p.s)
      ctx.restore()
    } else if (p.kind === 'bubbles') {
      ctx.beginPath()
      ctx.arc(p.x, p.y, 5 * p.s, 0, Math.PI * 2)
      ctx.strokeStyle = 'rgba(255,255,255,0.9)'
      ctx.lineWidth = 1.5
      ctx.stroke()
      ctx.fillStyle = 'rgba(255,255,255,0.15)'
      ctx.fill()
    } else {
      ctx.save()
      ctx.translate(p.x, p.y)
      ctx.rotate(p.life * 3)
      ctx.fillStyle = p.color
      const s = 6 * p.s * (1 - k)
      ctx.beginPath()
      for (let i = 0; i < 4; i++) {
        ctx.rotate(Math.PI / 2)
        ctx.moveTo(0, 0)
        ctx.quadraticCurveTo(s * 0.25, s * 0.25, 0, s)
        ctx.quadraticCurveTo(-s * 0.25, s * 0.25, 0, 0)
      }
      ctx.fill()
      ctx.restore()
    }
  }
  ctx.globalAlpha = 1
  if (parts.length) raf = requestAnimationFrame(tick)
  else {
    raf = 0
    ctx.clearRect(0, 0, innerWidth, innerHeight)
  }
}

export function burst(kind: Kind, x = innerWidth / 2, y = innerHeight / 3, count = 0) {
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return
  ensure()
  const colors = [cssVar('--accent', '#00b3b8'), cssVar('--accent2', '#ffb703'), cssVar('--gold', '#ffd166'), '#ffffff', '#ff7aa2']
  const n = count || (kind === 'confetti' ? 90 : kind === 'feathers' ? 18 : kind === 'bubbles' ? 14 : 16)
  for (let i = 0; i < n; i++) {
    const a = Math.random() * Math.PI * 2
    const sp = kind === 'confetti' ? 250 + Math.random() * 380 : kind === 'sparkles' ? 120 + Math.random() * 160 : 60 + Math.random() * 120
    parts.push({
      x, y,
      vx: Math.cos(a) * sp,
      vy: kind === 'confetti' ? -Math.abs(Math.sin(a) * sp) - 150 : kind === 'feathers' ? -60 - Math.random() * 120 : kind === 'bubbles' ? -40 - Math.random() * 40 : Math.sin(a) * sp,
      r: Math.random() * 6,
      vr: (Math.random() - 0.5) * 12,
      s: 0.7 + Math.random() * 0.8,
      life: 0,
      max: kind === 'feathers' ? 2.6 + Math.random() * 1.2 : kind === 'confetti' ? 2.2 + Math.random() : 1 + Math.random() * 0.6,
      color: colors[i % colors.length],
      kind,
      sway: 30 + Math.random() * 40,
      phase: Math.random() * 6,
    })
  }
  if (!raf) {
    last = performance.now()
    raf = requestAnimationFrame(tick)
  }
}

export function burstFrom(el: Element | null, kind: Kind, count = 0) {
  if (!el) return burst(kind)
  const r = el.getBoundingClientRect()
  burst(kind, r.left + r.width / 2, r.top + r.height / 2, count)
}
