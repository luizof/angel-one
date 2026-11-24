import type { Band, Dir } from './types'
import { DIRS } from './types'

export const TZ = 'America/Sao_Paulo'

const nf1 = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 1 })
const nf2 = new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 2 })
export const fmt1 = (n: number) => nf1.format(n)
export const fmt2 = (n: number) => nf2.format(n)

/** Hora local (Brasília) como número decimal, ex.: 17.5 = 17:30 */
export function localHour(d = new Date()): number {
  const p = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit', hour12: false, timeZone: TZ }).formatToParts(d)
  const h = Number(p.find((x) => x.type === 'hour')?.value ?? 0) % 24
  const m = Number(p.find((x) => x.type === 'minute')?.value ?? 0)
  return h + m / 60
}

/** "2026-11-19T09:00" na hora local de Brasília (mesmo formato do Open-Meteo). */
export function localHourKey(d = new Date()): string {
  const p = new Intl.DateTimeFormat('sv-SE', {
    year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', hour12: false, timeZone: TZ,
  }).format(d)
  // "2026-11-19 09"
  const [date, hh] = p.split(' ')
  return `${date}T${hh.padStart(2, '0')}:00`
}

export function bandOf(h = localHour()): Band {
  if (h >= 5 && h < 12) return 'manha'
  if (h >= 12 && h < 17) return 'tarde'
  if (h >= 17 && h < 20) return 'final_de_tarde'
  return 'noite'
}

export const BAND_INFO: Record<Band, { nome: string; saudacao: string; frase: string; theme: string }> = {
  manha: { nome: 'Manhã', saudacao: 'Bom dia a bordo', frase: 'Mar calmo, café passado e o dia inteiro pela frente.', theme: '#f7c9a8' },
  tarde: { nome: 'Tarde', saudacao: 'Boa tarde a bordo', frase: 'Sol a pino, água turquesa. Hora de um mergulho.', theme: '#1fb8c4' },
  final_de_tarde: { nome: 'Final de tarde', saudacao: 'Fim de tarde dourado', frase: 'O céu pegando fogo e um drink gelado na mão.', theme: '#f08a5d' },
  noite: { nome: 'Noite', saudacao: 'Boa noite a bordo', frase: 'Estrelas em cima, luzes azuis embaixo.', theme: '#0a1330' },
}

export function haversine(a: [number, number], b: [number, number]): number {
  const R = 6371008.8
  const toRad = (x: number) => (x * Math.PI) / 180
  const dLat = toRad(b[0] - a[0])
  const dLon = toRad(b[1] - a[1])
  const s = Math.sin(dLat / 2) ** 2 + Math.cos(toRad(a[0])) * Math.cos(toRad(b[0])) * Math.sin(dLon / 2) ** 2
  return 2 * R * Math.asin(Math.sqrt(s))
}

export function fmtKm(m: number): string {
  if (m < 1000) return `${Math.round(m / 10) * 10} m`
  return `${m < 10000 ? fmt1(m / 1000) : Math.round(m / 1000)} km`
}

export function fmtEta(m: number, kn: number): string {
  if (!kn) return ''
  const min = (m / 1852 / kn) * 60
  if (min < 1) return '< 1 min'
  if (min < 60) return `${Math.round(min)} min`
  const h = Math.floor(min / 60)
  const r = Math.round(min % 60)
  return r ? `${h} h ${r} min` : `${h} h`
}

export function dirOf(deg: number): Dir {
  return DIRS[Math.round((((deg % 360) + 360) % 360) / 45) % 8]
}

export const DIR_NOME: Record<Dir, string> = {
  N: 'Norte', NE: 'Nordeste', L: 'Leste', SE: 'Sudeste', S: 'Sul', SO: 'Sudoeste', O: 'Oeste', NO: 'Noroeste',
}
export const DIR_DEG: Record<Dir, number> = { N: 0, NE: 45, L: 90, SE: 135, S: 180, SO: 225, O: 270, NO: 315 }

export function timeAgo(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return ''
  const s = Math.max(0, (now - new Date(iso).getTime()) / 1000)
  if (s < 45) return 'agora'
  if (s < 3600) return `há ${Math.round(s / 60)} min`
  if (s < 86400) return `há ${Math.round(s / 3600)} h`
  return `há ${Math.round(s / 86400)} d`
}

export function hhmm(iso: string): string {
  return new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit', timeZone: TZ }).format(new Date(iso))
}

export function todayLong(): string {
  const s = new Intl.DateTimeFormat('pt-BR', { weekday: 'long', day: 'numeric', month: 'long', timeZone: TZ }).format(new Date())
  return s.charAt(0).toUpperCase() + s.slice(1)
}

export const isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1)
export const isStandalone = () =>
  (window.matchMedia?.('(display-mode: standalone)').matches ?? false) || (navigator as any).standalone === true

export function clamp(n: number, a: number, b: number) {
  return Math.max(a, Math.min(b, n))
}

export function mapsLinks(lat: number, lon: number, name: string) {
  const q = encodeURIComponent(name)
  return {
    google: `https://www.google.com/maps/search/?api=1&query=${lat},${lon}`,
    apple: `https://maps.apple.com/?ll=${lat},${lon}&q=${q}`,
  }
}

export function plural(n: number, um: string, varios: string) {
  return `${fmt2(n)} ${Math.abs(n) === 1 ? um : varios}`
}
