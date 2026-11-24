<script lang="ts">
  import { onMount } from 'svelte'
  import { app } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { BAND_INFO, todayLong, fmtKm, isIOS, isStandalone } from '../lib/util'
  import { forecastAhead, scoreDestination, RATING_INFO } from '../lib/wind'
  import { burst } from '../lib/fx'
  import { sparkle } from '../lib/sound'
  import Logo from '../components/Logo.svelte'
  import Waves from '../components/Waves.svelte'
  import BoatStatus from '../components/BoatStatus.svelte'
  import WindCard from '../components/WindCard.svelte'
  import OrderTracker from '../components/OrderTracker.svelte'
  import LowPolyIsland from '../components/LowPolyIsland.svelte'
  import Icon from '../components/Icon.svelte'

  let y = $state(0)
  onMount(() => {
    let raf = 0
    const on = () => {
      cancelAnimationFrame(raf)
      raf = requestAnimationFrame(() => (y = window.scrollY))
    }
    window.addEventListener('scroll', on, { passive: true })
    return () => window.removeEventListener('scroll', on)
  })

  const band = $derived(app.activeBand)
  const info = $derived(BAND_INFO[band])
  const f = $derived(forecastAhead(app.weather))
  const ranked = $derived(
    app.destinations
      .map((d) => ({ d, sc: scoreDestination(d, f), dist: app.distanceTo(d) }))
      .sort((a, b) => b.sc.score - a.sc.score || (a.dist?.m ?? 1e9) - (b.dist?.m ?? 1e9)),
  )
  const best = $derived(ranked.find(x => x.sc.rating !== 'sem_dados'))
  const totalVotes = $derived(app.destinations.reduce((s, d) => s + (d.votes || 0), 0))
  const topVoted = $derived([...app.destinations].filter((d) => d.votes > 0).sort((a, b) => b.votes - a.votes).slice(0, 3))
  const drinks = $derived(app.menu.filter((m) => m.available).slice(0, 10))
  const pendingOrders = $derived(app.orders.filter((o) => o.status === 'recebido' || o.status === 'preparando').length)
  const lowStock = $derived(
    app.inventory.items.filter((i) => i.min_qty != null && i.quantity < i.min_qty).length + app.inventory.tags.filter((t) => t.below_min).length,
  )
  const showInstall = $derived(isIOS && !isStandalone() && !localStorage.getItem('ao.hideInstall'))
  let hideInstall = $state(false)

  function secret() {
    if (app.isCrew) {
      router.go('/ajustes')
      return
    }
    sparkle()
    burst('feathers', innerWidth / 2, 150, 14)
    app.crewSheet = true
  }
</script>

<div class="home">
  <header class="cover">
    {#key band}
      <div class="photo" style="background-image:url(/capas/{band}.webp); transform: translate3d(0,{y * 0.38}px,0) scale({1.08 + Math.min(y, 300) / 2500})"></div>
    {/key}
    <div class="shade"></div>
    <div class="topbar">
      <BoatStatus />
      <span class="grow"></span>
      {#if app.isCrew}
        <a class="crew-btn" href="/ajustes" aria-label="Abrir configurações da tripulação">
          <span>{app.crew?.emoji}</span>
          <Icon name="gear" size={18} />
        </a>
      {/if}
    </div>
    <div class="logo-slot" style="opacity:{Math.max(0, 1 - y / 260)}; transform: translateY({y * 0.15}px) scale({1 - Math.min(y, 260) / 1400})">
      <Logo size={128} onsecret={secret} />
    </div>
    <Waves height={64} />
  </header>

  <div class="content stagger">
    <div class="greet">
      <div class="greet-top">
        <div class="date">{todayLong()}</div>
        {#if app.isCrew}<a href="/ajustes" class="settings-shortcut" aria-label="Abrir ajustes">⚙ Ajustes</a>{/if}
      </div>
      <h1>{info.saudacao}{app.crew ? `, ${app.crew.name}` : ''}!</h1>
      <p class="muted">{info.frase}</p>
    </div>

    {#if app.geoState === 'prompt' || app.geoState === 'unknown'}
      <button class="geo card" onclick={() => app.startGeo()}>
        <span class="geo-ico">📍</span>
        <span class="grow">
          <b>Ativar localização</b>
          <small>Para ver a distância até cada praia e mostrar se o barco está navegando.</small>
        </span>
        <span class="go">→</span>
      </button>
    {/if}

    <OrderTracker />

    {#if app.isCrew}
      <div class="crew-panel">
        <button class="cp card" onclick={() => router.go('/pedidos')}>
          <span class="n" class:hot={pendingOrders > 0}>{pendingOrders}</span>
          <span>comanda{pendingOrders === 1 ? '' : 's'} aberta{pendingOrders === 1 ? '' : 's'}</span>
        </button>
        <button class="cp card" onclick={() => router.go('/despensa/compras')}>
          <span class="n" class:hot={lowStock > 0}>{lowStock}</span>
          <span>na lista de compras</span>
        </button>
      </div>
    {/if}

    <section>
      <div class="section-h"><h2>Vento e mar</h2><span class="small muted">Jurerê · Open-Meteo</span></div>
      <WindCard />
    </section>

    {#if best}
      <section>
        <div class="section-h">
          <h2>Melhor parada agora</h2>
          <button class="link" onclick={() => router.go('/destinos')}>ver todas</button>
        </div>
        <button class="best card" onclick={() => router.go(`/destinos/${best.d.id}`)}>
          <div class="bimg" style="background-image:url({best.d.photo ?? '/capas/' + band + '.webp'})">
            <span class="rating r-{best.sc.rating}">{RATING_INFO[best.sc.rating].icon} {RATING_INFO[best.sc.rating].label}</span>
          </div>
          <div class="binfo">
            <div>
              <h3>{best.d.name}</h3>
              <div class="small muted">
                {#if best.dist}{fmtKm(best.dist.m)}{best.dist.water ? ' pela água' : ''} · {/if}{best.d.votes} voto{best.d.votes === 1 ? '' : 's'}
              </div>
            </div>
            <span class="arrow">→</span>
          </div>
        </button>
      </section>
    {/if}

    <section>
      <div class="section-h">
        <h2>Para onde vamos?</h2>
        <button class="link" onclick={() => router.go('/destinos')}>votar</button>
      </div>
      <div class="votes card pad">
        {#if topVoted.length}
          {#each topVoted as d, i (d.id)}
            <div class="vrow">
              <span class="medal">{['🥇', '🥈', '🥉'][i]}</span>
              <div class="grow">
                <div class="vname">{d.name}</div>
                <div class="vbar"><i style="width:{(d.votes / Math.max(1, totalVotes)) * 100}%"></i></div>
              </div>
              <b class="vn">{d.votes}</b>
            </div>
          {/each}
        {:else}
          <div class="vempty">
            <span>🗳️</span>
            <p>Ninguém votou ainda. <button class="link" onclick={() => router.go('/destinos')}>Escolha a próxima praia!</button></p>
          </div>
        {/if}
      </div>
    </section>

    <section>
      <div class="section-h">
        <h2>Bar do Anjo</h2>
        <button class="link" onclick={() => router.go('/bar')}>cardápio</button>
      </div>
      {#if drinks.length}
        <div class="carousel">
          {#each drinks as m (m.id)}
            <button class="dk" onclick={() => router.go(`/bar/${m.id}`)}>
              <div class="dimg" style="background-image:url({m.photo ?? ''})">{#if !m.photo}<span>🍹</span>{/if}</div>
              <span class="dn">{m.name}</span>
            </button>
          {/each}
        </div>
      {:else}
        <div class="card pad empty small">O cardápio ainda está vazio. {app.isCrew ? 'Cadastre os drinks no Bar.' : ''}</div>
      {/if}
    </section>

    {#if showInstall && !hideInstall}
      <div class="install card pad">
        <div class="row">
          <img src="/icons/apple-touch-icon.png" alt="" width="52" height="52" />
          <div class="grow">
            <b>Coloque o Angel One na tela inicial</b>
            <p class="small muted">Toque em <b>Compartilhar</b> <span class="share">⬆︎</span> e depois em <b>Adicionar à Tela de Início</b>. Assim chegam os avisos de drink pronto.</p>
          </div>
        </div>
        <button class="x" onclick={() => { localStorage.setItem('ao.hideInstall', '1'); hideInstall = true }} aria-label="Fechar">✕</button>
      </div>
    {/if}

    <footer class="foot">
      <LowPolyIsland />
      <p>feito com asas ✦ Angel One <span class="version">v{__APP_VERSION__}</span></p>
    </footer>
  </div>
</div>

<style>
  .cover {
    position: relative;
    height: min(72vh, 650px);
    min-height: 420px;
    overflow: hidden;
    background: var(--bg2);
  }
  .photo {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center top;
    animation: kenburns 26s ease-in-out infinite alternate, fadein 1s ease both;
    will-change: transform;
  }
  @keyframes kenburns {
    from {
      background-position: 45% 40%;
    }
    to {
      background-position: 55% 45%;
    }
  }
  @keyframes fadein {
    from {
      opacity: 0;
    }
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, color-mix(in srgb, var(--bg) 8%, transparent), transparent 32%);
    pointer-events: none;
  }
  .topbar {
    position: absolute;
    top: calc(var(--safe-top) + 16px);
    left: 16px;
    right: 16px;
    display: flex;
    align-items: center;
    gap: 8px;
    z-index: 3;
    min-height: 52px;
  }
  .crew-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 52px;
    height: 52px;
    padding: 0 15px;
    border-radius: 99px;
    color: #fff;
    background: rgba(10, 25, 40, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.25);
    -webkit-backdrop-filter: blur(14px);
    backdrop-filter: blur(14px);
    touch-action: manipulation;
    position: relative;
    z-index: 20;
    pointer-events: auto;
    text-decoration: none;
  }
  .logo-slot {
    position: absolute;
    top: calc(var(--safe-top) + 102px);
    left: 0;
    right: 0;
    display: flex;
    justify-content: center;
    z-index: 2;
  }
  .content {
    position: relative;
    z-index: 2;
    max-width: 640px;
    margin: -8px auto 0;
    padding: 0 18px calc(var(--tabbar-h) + var(--safe-bottom) + 24px);
  }
  .greet {
    padding: 0 4px 6px;
  }
  .date {
    font-size: 13px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--accent);
  }
  .greet-top { display: flex; justify-content: space-between; align-items: center; gap: 10px; }
  .settings-shortcut { font-size: 12px; font-weight: 800; color: var(--accent); text-decoration: none; padding: 10px 4px; }
  .greet h1 {
    font-size: 31px;
    margin: 4px 0 6px;
  }
  .geo {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 14px 16px;
    margin: 14px 0 4px;
    text-align: left;
  }
  .geo-ico {
    font-size: 26px;
    animation: bob 2.6s ease-in-out infinite;
  }
  .geo b {
    display: block;
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 17px;
  }
  .geo small {
    color: var(--muted);
    font-size: 13px;
  }
  .go,
  .arrow {
    font-weight: 900;
    color: var(--accent);
    font-size: 20px;
  }
  :global(.home .tracker) {
    margin-top: 14px;
  }
  .crew-panel {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-top: 14px;
  }
  .cp {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 14px;
    text-align: left;
    font-weight: 700;
    color: var(--muted);
    font-size: 14px;
  }
  .cp .n {
    font-family: var(--font-display);
    font-size: 34px;
    font-weight: 600;
    color: var(--ink);
    line-height: 1;
  }
  .cp .n.hot {
    color: var(--accent);
  }
  .best {
    display: block;
    width: 100%;
    overflow: hidden;
    text-align: left;
    transition: transform 0.3s var(--spring);
  }
  .best:active {
    transform: scale(0.98);
  }
  .bimg {
    position: relative;
    height: 170px;
    background-size: cover;
    background-position: center;
  }
  .rating {
    position: absolute;
    left: 12px;
    top: 12px;
    padding: 6px 12px;
    border-radius: 99px;
    font-weight: 900;
    font-size: 13px;
    color: #fff;
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
  }
  .r-otimo {
    background: color-mix(in srgb, var(--good) 85%, transparent);
  }
  .r-bom {
    background: color-mix(in srgb, var(--warn) 88%, transparent);
  }
  .r-exposto {
    background: color-mix(in srgb, var(--bad) 85%, transparent);
  }
  .r-sem_dados {
    background: rgba(30, 40, 60, 0.6);
  }
  .binfo {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
  }
  .binfo h3 {
    font-size: 21px;
  }
  .vrow {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 0;
  }
  .medal {
    font-size: 24px;
  }
  .vname {
    font-weight: 800;
  }
  .vbar {
    height: 8px;
    border-radius: 8px;
    background: var(--line);
    overflow: hidden;
    margin-top: 5px;
  }
  .vbar i {
    display: block;
    height: 100%;
    border-radius: 8px;
    background: linear-gradient(90deg, var(--accent), var(--accent2));
    animation: growx 1s var(--ease-out) both;
    transform-origin: left;
  }
  @keyframes growx {
    from {
      transform: scaleX(0);
    }
  }
  .vn {
    font-family: var(--font-display);
    font-size: 22px;
  }
  .vempty {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--muted);
  }
  .vempty span {
    font-size: 30px;
  }
  .link {
    color: var(--accent);
    font-weight: 800;
  }
  .carousel {
    display: flex;
    gap: 12px;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    scrollbar-width: none;
    padding: 4px 18px 10px;
    margin: 0 -18px;
  }
  .carousel::-webkit-scrollbar {
    display: none;
  }
  .dk {
    flex: none;
    width: 128px;
    scroll-snap-align: start;
    text-align: left;
    transition: transform 0.3s var(--spring);
  }
  .dk:active {
    transform: scale(0.95) rotate(-1deg);
  }
  .dimg {
    width: 128px;
    height: 150px;
    border-radius: 22px;
    background: var(--card-2) center/cover;
    box-shadow: var(--shadow);
    display: grid;
    place-items: center;
    font-size: 40px;
  }
  .dn {
    display: block;
    margin-top: 7px;
    font-weight: 800;
    font-size: 14px;
    line-height: 1.2;
  }
  .install {
    position: relative;
    margin-top: 22px;
  }
  .install img {
    border-radius: 13px;
  }
  .install b {
    font-family: var(--font-display);
    font-weight: 600;
  }
  .share {
    display: inline-block;
    padding: 0 4px;
    border-radius: 5px;
    background: var(--line);
  }
  .x {
    position: absolute;
    top: 8px;
    right: 12px;
    color: var(--muted);
  }
  .foot {
    margin-top: 30px;
    text-align: center;
    color: var(--muted);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
  }
  .foot p {
    margin-top: 4px;
  }
  .version { font-size: 10px; font-weight: 600; opacity: .5; margin-left: 5px; }
</style>
