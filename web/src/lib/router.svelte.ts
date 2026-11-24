import { tick } from 'svelte'

export const TAB_ORDER = ['/', '/destinos', '/bar', '/pedidos', '/despensa', '/ajustes']

function tabIndex(p: string) {
  const base = '/' + (p.split('/')[1] ?? '')
  const i = TAB_ORDER.indexOf(base === '/' ? '/' : base)
  return i < 0 ? 0 : i
}

class Router {
  path = $state(location.pathname)

  constructor() {
    if ('scrollRestoration' in history) history.scrollRestoration = 'manual'
    window.addEventListener('popstate', () => this.set(location.pathname))
    window.addEventListener('ao:navigate', (e) => this.go((e as CustomEvent).detail))
  }

  get section() {
    return '/' + (this.path.split('/')[1] ?? '')
  }

  private set(p: string) {
    if (p === this.path) return
    const dir = tabIndex(p) >= tabIndex(this.path) ? 'forward' : 'back'
    const root = document.documentElement
    root.dataset.navdir = dir
    this.path = p
    window.scrollTo(0, 0)
    // A tela anterior pode ser maior: a rolagem deve ocorrer também depois
    // da troca efetiva do DOM, especialmente no Safari em modo standalone.
    void tick().then(() => window.scrollTo(0, 0))
  }

  go(p: string, replace = false) {
    if (p === this.path) {
      window.scrollTo({ top: 0, behavior: 'smooth' })
      return
    }
    if (replace) history.replaceState(null, '', p)
    else history.pushState(null, '', p)
    this.set(p)
  }
}

export const router = new Router()
