// Avaliação dos destinos pela previsão de vento/ondulação.
import type { Destination, Dir, Weather, WeatherHour } from './types'
import { DIR_DEG, localHourKey } from './util'

export type Rating = 'otimo' | 'bom' | 'exposto' | 'sem_dados'

export const RATING_INFO: Record<Rating, { label: string; icon: string }> = {
  otimo: { label: 'Ótimo', icon: '🪽' },
  bom: { label: 'Bom', icon: '👍' },
  exposto: { label: 'Exposto', icon: '🌊' },
  sem_dados: { label: 'Sem avaliação', icon: '❔' },
}

const CALM_KN = 7

function angDiff(a: number, b: number) {
  const d = Math.abs((((a - b) % 360) + 540) % 360 - 180)
  return d
}

/** 1 = abrigado, 0 = totalmente exposto. */
function protection(dir: number, good: Dir[]): number {
  let best = 180
  for (const g of good) best = Math.min(best, angDiff(dir, DIR_DEG[g]))
  if (best <= 22.5) return 1
  if (best <= 45) return 0.65
  return 0
}

export function hourScore(h: WeatherHour, good: Dir[]): number {
  const kn = Math.max(h.wind_kn ?? 0, (h.gust_kn ?? 0) * 0.65)
  let s: number
  if (kn < CALM_KN) s = 1
  else if (h.wind_dir == null) s = 0.4
  else {
    const p = protection(h.wind_dir, good)
    const exposed = kn < 12 ? 0.55 : kn < 18 ? 0.25 : 0.05
    s = p + (1 - p) * exposed
  }
  const swell = h.swell_m ?? h.wave_m
  const sdir = h.swell_dir ?? h.wave_dir
  if (swell != null && sdir != null && swell > 0.5) {
    const p = protection(sdir, good)
    const k = swell > 1.2 ? 0.45 : 0.7
    s *= p + (1 - p) * k
  }
  return s
}

export interface Forecast {
  now: WeatherHour | null
  /** horas de agora até o fim do dia (mín. 12 h) */
  ahead: WeatherHour[]
}

export function forecastAhead(w: Weather | null): Forecast {
  if (!w || !w.hours?.length) return { now: null, ahead: [] }
  const key = localHourKey()
  let i = w.hours.findIndex((h) => h.time >= key)
  if (i < 0 || w.hours[i].time !== key) return { now: null, ahead: [] }
  const today = key.slice(0, 10)
  let end = i
  while (end < w.hours.length && (w.hours[end].time.slice(0, 10) === today || end - i < 12)) end++
  return { now: w.hours[i], ahead: w.hours.slice(i, Math.max(end, i + 1)) }
}

export interface DestScore {
  score: number
  rating: Rating
  hours: { time: string; s: number }[]
}

export function scoreDestination(d: Destination, f: Forecast): DestScore {
  if (!f.ahead.length || f.now?.wind_kn == null) return { score: 0, rating: 'sem_dados', hours: [] }
  const valid = f.ahead.filter(h => h.wind_kn != null)
  const calmAll = valid.every((h) => Math.max(h.wind_kn!, (h.gust_kn ?? 0) * 0.65) < CALM_KN && ((h.swell_m ?? h.wave_m ?? 0) <= 0.5))
  if (!d.good_winds?.length && !calmAll) {
    return { score: 0, rating: 'sem_dados', hours: [] }
  }
  let num = 0
  let den = 0
  const hours = valid.map((h, idx) => {
    const s = hourScore(h, d.good_winds ?? [])
    const w = Math.exp(-idx / 4)
    num += s * w
    den += w
    return { time: h.time, s }
  })
  const score = den ? num / den : 0
  const rating: Rating = score >= 0.78 ? 'otimo' : score >= 0.5 ? 'bom' : 'exposto'
  return { score, rating, hours }
}

export function windColor(kn: number | null): string {
  const k = kn ?? 0
  if (k < 7) return 'var(--good)'
  if (k < 13) return 'var(--accent2)'
  if (k < 19) return 'var(--warn)'
  return 'var(--bad)'
}

export function scoreColor(s: number): string {
  if (s >= 0.78) return 'var(--good)'
  if (s >= 0.5) return 'var(--warn)'
  return 'var(--bad)'
}
