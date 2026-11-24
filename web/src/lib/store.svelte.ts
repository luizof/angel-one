// Estado global reativo + tempo real (SSE) + GPS.
import { api, ApiError, setCrewHeader } from './api'
import type {
  AiModel, Band, BoatStatus, Crew, Destination, Inventory, MenuItem, Order, Route, Settings, Weather,
} from './types'
import { bandOf, haversine, BAND_INFO } from './util'
import { chime, tada } from './sound'
import { burst } from './fx'

export interface Toast {
  id: number
  text: string
  kind: 'ok' | 'err' | 'info'
  icon?: string
}

class AppState {
  // identidade
  crew = $state<Crew | null>(null)
  get isCrew() {
    return this.crew != null
  }

  // tema
  band = $state<Band>(bandOf())
  bandPreview = $state<Band | null>((localStorage.getItem('ao.bandPreview') as Band) || null)

  // dados
  settings = $state<Settings | null>(null)
  crewList = $state<Crew[]>([])
  menu = $state<MenuItem[]>([])
  orders = $state<Order[]>([])
  inventory = $state<Inventory>({ items: [], tags: [] })
  destinations = $state<Destination[]>([])
  weather = $state<Weather | null>(null)
  status = $state<BoatStatus | null>(null)
  routes = $state<Record<number, Route>>({})
  models = $state<{ text: AiModel[]; image: AiModel[] } | null>(null)

  // localização deste aparelho
  myPos = $state<{ lat: number; lon: number; acc: number; at: number } | null>(null)
  geoState = $state<'unknown' | 'granted' | 'denied' | 'prompt' | 'unsupported'>('unknown')

  // ui
  toasts = $state<Toast[]>([])
  online = $state(true)
  loaded = $state(false)
  crewSheet = $state(false)

  private es: EventSource | null = null
  private routesKey = ''
  private geoTimer = 0
  private lastPost = 0
  private orderRequest = 0
  private latestCrewOrder: number | null = null

  constructor() {
    try {
      const c = localStorage.getItem('ao.crew')
      if (c) this.crew = JSON.parse(c)
    } catch {
      this.crew = null
    }
    setCrewHeader(this.crew?.id ?? null)
  }

  // ---------- tema ----------
  get activeBand(): Band {
    return this.bandPreview ?? this.band
  }
  tickBand() {
    this.band = bandOf()
    this.applyBand()
  }
  applyBand() {
    const b = this.activeBand
    document.documentElement.dataset.band = b
    document.querySelector('meta[name="theme-color"]')?.setAttribute('content', BAND_INFO[b].theme)
  }
  previewBand(b: Band | null) {
    this.bandPreview = b
    if (b) localStorage.setItem('ao.bandPreview', b)
    else localStorage.removeItem('ao.bandPreview')
    this.applyBand()
  }

  // ---------- toasts ----------
  toast(text: string, kind: Toast['kind'] = 'ok', icon?: string) {
    const id = Date.now() + Math.random()
    this.toasts = [...this.toasts, { id, text, kind, icon }]
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === 'err' ? 4800 : 3000)
  }
  fail(e: unknown) {
    const msg = e instanceof ApiError ? e.message : e instanceof Error ? e.message : 'Algo deu errado.'
    this.toast(msg, 'err', '⚠️')
  }

  // ---------- tripulação ----------
  becomeCrew(c: Crew) {
    this.crew = c
    localStorage.setItem('ao.crew', JSON.stringify(c))
    setCrewHeader(c.id)
    this.latestCrewOrder = null
    this.checkPush().catch(() => {})
    this.loadOrders()
  }
  leaveCrew() {
    this.crew = null
    localStorage.removeItem('ao.crew')
    setCrewHeader(null)
    this.latestCrewOrder = null
    this.orders = []
    this.checkPush().catch(() => {})
    this.loadOrders()
  }

  // ---------- carregamento ----------
  async init() {
    this.applyBand()
    setInterval(() => this.tickBand(), 30_000)
    await Promise.allSettled([
      this.loadSettings(), this.loadMenu(), this.loadOrders(), this.loadInventory(), this.loadDestinations(),
      this.loadStatus(), this.loadCrew(),
    ])
    this.loaded = true
    this.loadWeather()
    // Quick Tunnels podem armazenar SSE em buffer: reconciliar também por HTTP.
    setInterval(() => {
      if (document.visibilityState !== 'visible') return
      this.loadOrders()
      this.loadDestinations()
      this.loadStatus()
      this.loadInventory()
      this.loadMenu()
    }, 8000)
    this.connect()
    this.initGeo()
    setInterval(() => this.loadWeather(), 20 * 60_000)
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') {
        this.connect(true)
        this.refreshAll()
        if (this.geoState === 'granted') this.sampleGeo()
      }
    })
    navigator.serviceWorker?.addEventListener('message', (ev) => {
      if (ev.data?.type === 'navigate' && typeof ev.data.url === 'string') {
        window.dispatchEvent(new CustomEvent('ao:navigate', { detail: ev.data.url }))
      }
    })
  }

  refreshAll() {
    this.loadMenu()
    this.loadOrders()
    this.loadInventory()
    this.loadDestinations()
    this.loadStatus()
  }

  async loadSettings() {
    try {
      this.settings = await api<Settings>('/api/settings')
    } catch {
      /* mantém */
    }
  }
  async loadCrew() {
    try {
      this.crewList = await api<Crew[]>('/api/crew')
      if (this.crew && !this.crewList.some((c) => c.id === this.crew!.id)) this.leaveCrew()
    } catch {
      /* */
    }
  }
  async loadMenu() {
    try {
      const menu = await api<MenuItem[]>('/api/menu')
      if (JSON.stringify(this.menu) !== JSON.stringify(menu)) this.menu = menu
    } catch {
      /* */
    }
  }
  async loadOrders() {
    const request = ++this.orderRequest
    try {
      const prev = new Map(this.orders.map((o) => [o.id, o.status]))
      const list = await api<Order[]>('/api/orders')
      if (request !== this.orderRequest) return
      this.online = true
      if (this.isCrew) {
        if (this.latestCrewOrder !== null) {
          for (const o of list.filter(o => o.id > this.latestCrewOrder! && o.status === 'recebido')) {
            chime()
            this.toast(`Novo pedido: ${o.item_name} para ${o.for_name}`, 'info', '🍹')
          }
        }
        this.latestCrewOrder = Math.max(this.latestCrewOrder ?? 0, ...list.map(o => o.id))
      }
      if (!this.isCrew) {
        for (const o of list) {
          if (o.status === 'pronto' && prev.get(o.id) && prev.get(o.id) !== 'pronto') this.celebrateReady(o)
        }
      }
      this.orders = list
    } catch {
      if (request === this.orderRequest) this.online = false
    }
  }
  async loadInventory() {
    try {
      const inventory = await api<Inventory>('/api/inventory')
      if (JSON.stringify(this.inventory) !== JSON.stringify(inventory)) this.inventory = inventory
    } catch {
      /* */
    }
  }
  async loadDestinations() {
    try {
      const places = await api<Destination[]>('/api/destinations')
      if (JSON.stringify(this.destinations) !== JSON.stringify(places)) this.destinations = places
      this.updateRoutes()
    } catch {
      /* */
    }
  }
  async loadWeather() {
    try {
      this.weather = await api<Weather>('/api/weather')
    } catch {
      /* */
    }
  }
  async loadStatus() {
    try {
      this.status = await api<BoatStatus>('/api/status')
      this.updateRoutes()
    } catch {
      /* */
    }
  }
  async loadModels() {
    if (this.models) return this.models
    this.models = await api('/api/ai/models')
    return this.models
  }

  celebrateReady(o: Order) {
    tada()
    burst('confetti', innerWidth / 2, innerHeight * 0.35)
    this.toast(`${o.item_name} de ${o.for_name} está pronto!`, 'ok', '🪽')
  }

  // ---------- SSE ----------
  connect(force = false) {
    if (this.es && !force && this.es.readyState !== EventSource.CLOSED) return
    this.es?.close()
    const es = new EventSource('/api/events')
    this.es = es
    es.onopen = () => (this.online = true)
    const on = (name: string, fn: (d: any) => void) =>
      es.addEventListener(name, (ev) => {
        let d: any = {}
        try {
          d = JSON.parse((ev as MessageEvent).data || '{}')
        } catch {
          /* */
        }
        fn(d)
      })
    on('order_new', () => this.loadOrders())
    on('order_update', () => this.loadOrders())
    on('menu', () => this.loadMenu())
    on('inventory', () => this.loadInventory())
    on('destinations', () => this.loadDestinations())
    on('crew', () => this.loadCrew())
    on('settings', () => this.loadSettings())
    on('votes', (d: { votes: Record<string, number> }) => {
      if (!d?.votes) return
      this.destinations = this.destinations.map((x) => ({ ...x, votes: d.votes[String(x.id)] ?? 0 }))
    })
    on('status', (s: BoatStatus) => {
      this.status = s
      this.updateRoutes()
    })
  }

  // ---------- GPS ----------
  async initGeo() {
    if (!('geolocation' in navigator)) {
      this.geoState = 'unsupported'
      return
    }
    try {
      const p = await (navigator as any).permissions?.query({ name: 'geolocation' })
      if (p) {
        this.geoState = p.state
        p.onchange = () => {
          this.geoState = p.state
          if (p.state === 'granted') this.startGeo()
        }
      } else this.geoState = localStorage.getItem('ao.geo') === 'granted' ? 'granted' : 'prompt'
    } catch {
      this.geoState = localStorage.getItem('ao.geo') === 'granted' ? 'granted' : 'prompt'
    }
    if (this.geoState === 'granted') this.startGeo()
  }

  /** Chamado por um toque do usuário (o iOS mostra a permissão). */
  startGeo() {
    this.sampleGeo()
    this.scheduleGeo()
  }

  private scheduleGeo() {
    clearInterval(this.geoTimer)
    this.geoTimer = window.setInterval(() => {
      if (document.visibilityState === 'visible') this.sampleGeo()
    }, 60_000)
  }

  sampleGeo(): Promise<{ lat: number; lon: number } | null> {
    return new Promise((resolve) => {
      if (!('geolocation' in navigator) || this.geoState === 'denied') return resolve(null)
      navigator.geolocation.getCurrentPosition(
        (pos) => {
          this.geoState = 'granted'
          if (!this.geoTimer) this.scheduleGeo()
          localStorage.setItem('ao.geo', 'granted')
          const { latitude: lat, longitude: lon, accuracy, speed, heading } = pos.coords
          this.myPos = { lat, lon, acc: accuracy, at: Date.now() }
          this.updateRoutes()
          if (Date.now() - this.lastPost > 45_000 && accuracy <= 150) {
            this.lastPost = Date.now()
            api('/api/position', {
              body: {
                lat, lon, accuracy_m: accuracy,
                speed_mps: speed != null && speed >= 0 ? speed : null,
                heading: heading != null && !Number.isNaN(heading) ? heading : null,
              },
            }).catch(() => {})
          }
          resolve({ lat, lon })
        },
        (err) => {
          if (err.code === err.PERMISSION_DENIED) {
            this.geoState = 'denied'
            localStorage.removeItem('ao.geo')
          }
          resolve(null)
        },
        { enableHighAccuracy: true, maximumAge: 20_000, timeout: 20_000 },
      )
    })
  }

  /** Origem para as distâncias: GPS deste aparelho (recente) ou a última posição do barco. */
  get origin(): { lat: number; lon: number; source: 'eu' | 'barco' } | null {
    if (this.myPos && Date.now() - this.myPos.at < 10 * 60_000) return { lat: this.myPos.lat, lon: this.myPos.lon, source: 'eu' }
    if (this.status?.lat != null && this.status?.lon != null) return { lat: this.status.lat, lon: this.status.lon, source: 'barco' }
    if (this.myPos) return { lat: this.myPos.lat, lon: this.myPos.lon, source: 'eu' }
    return null
  }

  async updateRoutes() {
    const o = this.origin
    if (!o || !this.destinations.length) return
    const key = `${o.lat.toFixed(3)},${o.lon.toFixed(3)}|${this.destinations.map((d) => `${d.id}:${d.lat},${d.lon}`).join(';')}`
    if (key === this.routesKey) return
    this.routesKey = key
    try {
      const list = await api<Route[]>(`/api/routes?lat=${o.lat}&lon=${o.lon}`)
      const m: Record<number, Route> = {}
      for (const r of list) m[r.destination_id] = r
      this.routes = m
    } catch {
      this.routesKey = ''
    }
  }

  distanceTo(d: Destination): { m: number; water: boolean } | null {
    const r = this.routes[d.id]
    if (r) return { m: r.distance_m, water: r.via_water }
    const o = this.origin
    if (!o) return null
    return { m: haversine([o.lat, o.lon], [d.lat, d.lon]), water: false }
  }

  // ---------- votos ----------
  myVoteId = $derived(this.destinations.find((d) => d.my_vote)?.id ?? null)

  async vote(id: number) {
    const res = await api<{ votes: Record<string, number>; my_vote: number | null }>(`/api/destinations/${id}/vote`, { body: {} })
    this.applyVotes(res)
  }
  async unvote() {
    const res = await api<{ votes: Record<string, number>; my_vote: number | null }>('/api/votes/mine', { method: 'DELETE' })
    this.applyVotes(res)
  }
  private applyVotes(res: { votes: Record<string, number>; my_vote: number | null }) {
    this.destinations = this.destinations.map((x) => ({
      ...x, votes: res.votes[String(x.id)] ?? 0, my_vote: res.my_vote === x.id,
    }))
  }

  // ---------- push ----------
  pushState = $state<'unsupported' | 'default' | 'granted' | 'denied' | 'subscribed'>('default')

  async checkPush() {
    if (!('serviceWorker' in navigator) || !('PushManager' in window) || !('Notification' in window)) {
      this.pushState = 'unsupported'
      return
    }
    const reg = await navigator.serviceWorker.ready
    const sub = await reg.pushManager.getSubscription()
    if (sub && Notification.permission === 'granted') {
      this.pushState = 'subscribed'
      // reenviar mantém o vínculo com o tripulante atual
      api('/api/push/subscribe', { body: { subscription: sub.toJSON() } }).catch(() => {})
    } else this.pushState = Notification.permission as any
  }

  async enablePush(): Promise<boolean> {
    try {
      if (!('serviceWorker' in navigator) || !('PushManager' in window)) {
        this.pushState = 'unsupported'
        return false
      }
      const perm = await Notification.requestPermission()
      if (perm !== 'granted') {
        this.pushState = perm as any
        return false
      }
      if (!this.settings) await this.loadSettings()
      const key = this.settings?.vapid_public_key
      if (!key) throw new Error('Servidor sem chave de notificação.')
      const reg = await navigator.serviceWorker.ready
      let sub = await reg.pushManager.getSubscription()
      if (!sub) {
        sub = await reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: b64ToU8(key) as BufferSource })
      }
      await api('/api/push/subscribe', { body: { subscription: sub.toJSON() } })
      this.pushState = 'subscribed'
      return true
    } catch (e) {
      this.fail(e)
      return false
    }
  }
}

function b64ToU8(b64: string): Uint8Array {
  const pad = '='.repeat((4 - (b64.length % 4)) % 4)
  const s = atob((b64 + pad).replace(/-/g, '+').replace(/_/g, '/'))
  const out = new Uint8Array(s.length)
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i)
  return out
}

export const app = new AppState()
