// Sons sintetizados (WebAudio) — sem arquivos. O iOS exige um toque para liberar o áudio.
let ctx: AudioContext | null = null
let enabled = localStorage.getItem('ao.sound') !== 'off'

export function soundEnabled() {
  return enabled
}
export function setSoundEnabled(v: boolean) {
  enabled = v
  localStorage.setItem('ao.sound', v ? 'on' : 'off')
  if (v) unlock()
}

export function unlock() {
  try {
    if (!ctx) ctx = new (window.AudioContext || (window as any).webkitAudioContext)()
    if (ctx.state === 'suspended') ctx.resume()
    // toca um silêncio para "destravar" no iOS
    const b = ctx.createBuffer(1, 1, 22050)
    const s = ctx.createBufferSource()
    s.buffer = b
    s.connect(ctx.destination)
    s.start(0)
  } catch {
    /* sem áudio */
  }
}

export function audioReady() {
  return !!ctx && ctx.state === 'running'
}

function tone(freq: number, start: number, dur: number, gain = 0.18, type: OscillatorType = 'sine') {
  if (!ctx) return
  const t = ctx.currentTime + start
  const o = ctx.createOscillator()
  const g = ctx.createGain()
  o.type = type
  o.frequency.setValueAtTime(freq, t)
  g.gain.setValueAtTime(0.0001, t)
  g.gain.exponentialRampToValueAtTime(gain, t + 0.012)
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur)
  o.connect(g).connect(ctx.destination)
  o.start(t)
  o.stop(t + dur + 0.05)
}

function play(fn: () => void) {
  if (!enabled) return
  try {
    if (!ctx) unlock()
    if (ctx?.state === 'suspended') ctx.resume()
    fn()
  } catch {
    /* ignore */
  }
}

/** Sino de pedido novo (tripulação). */
export const chime = () =>
  play(() => {
    tone(1318.5, 0, 1.1, 0.2)
    tone(1975.5, 0.0, 0.9, 0.06)
    tone(1760, 0.18, 1.2, 0.18)
    tone(2637, 0.18, 0.8, 0.05)
  })

/** "Plim" curto (voto, toque). */
export const pop = () =>
  play(() => {
    tone(880, 0, 0.12, 0.12, 'triangle')
    tone(1320, 0.05, 0.14, 0.08, 'triangle')
  })

/** Arpejo alegre (pedido pronto, conferência concluída). */
export const tada = () =>
  play(() => {
    ;[784, 988, 1175, 1568].forEach((f, i) => tone(f, i * 0.09, 0.5, 0.14, 'triangle'))
  })

/** Brilho angelical (entrada da tripulação). */
export const sparkle = () =>
  play(() => {
    ;[2093, 2637, 3136, 2637, 3520].forEach((f, i) => tone(f, i * 0.06, 0.4, 0.05))
  })
