<script lang="ts">
  import { app } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { fmtKm, fmtEta, mapsLinks, DIR_NOME } from '../lib/util'
  import { forecastAhead, scoreDestination, scoreColor, RATING_INFO } from '../lib/wind'
  import { burstFrom } from '../lib/fx'
  import { pop } from '../lib/sound'
  import type { Destination } from '../lib/types'
  import MapView from '../components/MapView.svelte'
  import Sheet from '../components/Sheet.svelte'
  import WindRose from '../components/WindRose.svelte'
  import Icon from '../components/Icon.svelte'
  import DestinoForm from './DestinoForm.svelte'
  import { api } from '../lib/api'

  type Sort = 'vento' | 'perto' | 'votos'
  let sort = $state<Sort>((localStorage.getItem('ao.destSort') as Sort) || 'vento')
  $effect(() => localStorage.setItem('ao.destSort', sort))

  const f = $derived(forecastAhead(app.weather))
  const list = $derived(
    app.destinations
      .map((d) => ({ d, sc: scoreDestination(d, f), dist: app.distanceTo(d) }))
      .sort((a, b) => {
        if (sort === 'perto') return (a.dist?.m ?? 1e12) - (b.dist?.m ?? 1e12)
        if (sort === 'votos') return b.d.votes - a.d.votes || b.sc.score - a.sc.score
        return b.sc.score - a.sc.score || (a.dist?.m ?? 1e12) - (b.dist?.m ?? 1e12)
      }),
  )

  const openId = $derived(Number(router.path.split('/')[2]) || null)
  const open = $derived(openId != null ? list.find((x) => x.d.id === openId) : undefined)
  let sheetOpen = $state(false)
  $effect(() => {
    sheetOpen = !!open
  })
  let formOpen = $state(false)
  let editing = $state<Destination | null>(null)

  function select(id: number) {
    router.go(`/destinos/${id}`)
  }
  function closeDetail() {
    router.go('/destinos', true)
  }

  let voting = $state<number | null>(null)
  async function vote(d: Destination, e: Event) {
    e.stopPropagation()
    const anchor = e.currentTarget as Element
    voting = d.id
    try {
      if (d.my_vote) {
        await app.unvote()
      } else {
        await app.vote(d.id)
        pop()
        burstFrom(anchor, 'feathers', 10)
      }
    } catch (err) {
      app.fail(err)
    } finally {
      voting = null
    }
  }

  async function resetVotes() {
    if (!confirm('Zerar todos os votos?')) return
    try {
      await api('/api/votes/reset', { body: {} })
      app.toast('Votação zerada', 'info', '🗳️')
      app.loadDestinations()
    } catch (e) {
      app.fail(e)
    }
  }

  function newDest() {
    editing = null
    formOpen = true
  }
  function edit(d: Destination) {
    editing = d
    formOpen = true
  }

  const cruise = $derived(app.status?.moving && app.status.speed_kn ? app.status.speed_kn : (app.settings?.cruise_speed_kn ?? 18))
</script>

<div class="page">
  <div class="page-title">
    <div class="grow">
      <h1>Destinos</h1>
      <div class="sub">
        {#if app.origin}
          Distâncias a partir {app.origin.source === 'eu' ? 'de onde você está' : 'da última posição do barco'}
        {:else}
          <button class="lnk" onclick={() => app.startGeo()}>Ative a localização</button> para ver as distâncias
        {/if}
      </div>
    </div>
    {#if app.isCrew}
      <button class="icon-btn add" onclick={newDest} aria-label="Novo destino"><Icon name="plus" /></button>
    {/if}
  </div>

  <MapView destinations={app.destinations} selected={openId} onselect={select} height="38vh" />
  <p class="small muted" style="margin:8px 4px 0">Votos zeram à meia-noite e ao meio-dia (horário de Florianópolis).</p>

  <div class="chips sorts">
    <button class="chip" class:on={sort === 'vento'} onclick={() => (sort = 'vento')}>🌬️ Melhor com o vento</button>
    <button class="chip" class:on={sort === 'perto'} onclick={() => (sort = 'perto')}>📍 Mais perto</button>
    <button class="chip" class:on={sort === 'votos'} onclick={() => (sort = 'votos')}>🗳️ Mais votados</button>
  </div>

  {#if !list.length}
    <div class="empty card">
      <div class="art">🏝️</div>
      <p>Nenhum destino ainda.</p>
      {#if app.isCrew}<button class="btn" style="margin-top:14px" onclick={newDest}>Cadastrar o primeiro</button>{/if}
    </div>
  {/if}

  <div class="list stagger">
    {#each list as { d, sc, dist } (d.id)}
      <div class="dest card" role="button" tabindex="0" onclick={() => select(d.id)} onkeydown={(e) => e.key === 'Enter' && select(d.id)}>
        <div class="img" style="background-image:url({d.photo ?? '/capas/tarde.webp'})">
          <span class="rating r-{sc.rating}">{RATING_INFO[sc.rating].icon} {RATING_INFO[sc.rating].label}</span>
          <button class="vote" class:on={d.my_vote} onclick={(e) => vote(d, e)} disabled={voting === d.id} aria-label="Votar">
            <Icon name="wing" size={18} />
            <b>{d.votes}</b>
          </button>
        </div>
        <div class="info">
          <div class="grow">
            <h3>{d.name}</h3>
            <div class="meta">
              {#if dist}
                <span><b>{fmtKm(dist.m)}</b>{dist.water ? ' pela água' : ' em linha reta'}</span>
                <span class="dotsep">·</span>
                <span>{fmtEta(dist.m, cruise)}</span>
              {:else}
                <span class="muted">distância indisponível</span>
              {/if}
            </div>
          </div>
        </div>
        {#if sc.hours.length}
          <div class="timeline" title="Próximas horas">
            {#each sc.hours.slice(0, 16) as h}
              <i style="background:{scoreColor(h.s)}"></i>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </div>

  {#if app.isCrew && app.destinations.some((d) => d.votes > 0)}
    <button class="btn danger block" style="margin-top:18px" onclick={resetVotes}>Zerar votação</button>
  {/if}
</div>

<Sheet bind:open={sheetOpen} onclose={closeDetail}>
  {#snippet hero()}
    {#if open}
      <div class="hero" style="background-image:url({open.d.photo ?? '/capas/tarde.webp'})">
        <div class="hshade"></div>
        <h2>{open.d.name}</h2>
      </div>
    {/if}
  {/snippet}
  {#if open}
    {@const d = open.d}
    {@const links = mapsLinks(d.lat, d.lon, d.name)}
    <div class="dstats">
      <div class="st">
        <small>Distância</small>
        <b>{open.dist ? fmtKm(open.dist.m) : '—'}</b>
        <span>{open.dist ? (open.dist.water ? 'pela água' : 'linha reta') : ''}</span>
      </div>
      <div class="st">
        <small>Tempo</small>
        <b>{open.dist ? fmtEta(open.dist.m, cruise) : '—'}</b>
        <span>a {Math.round(cruise)} nós</span>
      </div>
      <div class="st">
        <small>Votos</small>
        <b>{d.votes}</b>
        <span>{d.my_vote ? 'inclui o seu' : ''}</span>
      </div>
    </div>

    <button class="btn block {d.my_vote ? 'ghost' : 'gold'}" style="margin:14px 0 6px" onclick={(e) => vote(d, e)}>
      <Icon name="wing" size={20} />
      {d.my_vote ? 'Tirar meu voto' : 'Votar para ir pra cá'}
    </button>

    <h3 class="sh">Abrigo do vento</h3>
    <div class="shelter">
      <WindRose value={d.good_winds} wind={f.now?.wind_dir ?? null} size={190} />
      <div class="grow small">
        <span class="pill r-{open.sc.rating}" style="color:{scoreColor(open.sc.score)}">{RATING_INFO[open.sc.rating].icon} {RATING_INFO[open.sc.rating].label}</span>
        <p style="margin-top:8px">
          {#if d.good_winds.length}
            Abrigado de ventos de <b>{d.good_winds.map((x) => DIR_NOME[x]).join(', ')}</b>.
          {:else}
            A tripulação ainda não marcou de quais ventos este lugar é abrigado.
          {/if}
        </p>
        <p class="muted" style="margin-top:6px">A seta mostra o vento de agora.</p>
      </div>
    </div>

    {#if open.sc.hours.length}
      <h3 class="sh">Próximas horas</h3>
      <div class="hours">
        {#each open.sc.hours.slice(0, 16) as h}
          <div class="hh">
            <i style="height:{12 + h.s * 34}px; background:{scoreColor(h.s)}"></i>
            <span>{h.time.slice(11, 13)}h</span>
          </div>
        {/each}
      </div>
    {/if}

    <h3 class="sh">Ver o ponto no mapa</h3>
    <p class="small muted" style="margin-bottom:8px">A distância pela água considera a costa, mas não a profundidade nem os obstáculos submersos.</p>
    <div class="nav">
      <a class="btn ghost sm" href={links.google} target="_blank" rel="noopener">Google Maps</a>
      <a class="btn ghost sm" href={links.apple} target="_blank" rel="noopener">Apple Mapas</a>
    </div>

    {#if app.isCrew}
      <div class="row" style="margin-top:20px">
        <button class="btn soft grow" onclick={() => edit(d)}><Icon name="edit" size={18} /> Editar</button>
      </div>
    {/if}
  {/if}
</Sheet>

{#if app.isCrew}
  <DestinoForm bind:open={formOpen} dest={editing} />
{/if}

<style>
  .lnk {
    color: var(--accent);
    font-weight: 800;
  }
  .add {
    background: var(--accent);
    color: var(--accent-ink);
    border: 0;
    box-shadow: 0 8px 18px -8px var(--accent);
  }
  .sorts {
    margin-top: 14px;
    margin-bottom: 4px;
  }
  .list {
    display: grid;
    gap: 14px;
    margin-top: 8px;
  }
  .dest {
    overflow: hidden;
    cursor: pointer;
    transition: transform 0.3s var(--spring);
  }
  .dest:active {
    transform: scale(0.985);
  }
  .img {
    position: relative;
    height: 150px;
    background-size: cover;
    background-position: center;
  }
  .rating {
    position: absolute;
    left: 10px;
    top: 10px;
    padding: 5px 11px;
    border-radius: 99px;
    color: #fff;
    font-weight: 900;
    font-size: 12.5px;
    -webkit-backdrop-filter: blur(8px);
    backdrop-filter: blur(8px);
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
  .vote {
    position: absolute;
    right: 10px;
    bottom: 10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 38px;
    padding: 0 14px;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.92);
    color: #3b2a00;
    font-size: 16px;
    box-shadow: 0 6px 16px -6px rgba(0, 0, 0, 0.4);
    transition: transform 0.3s var(--spring), background 0.25s;
  }
  .vote:active {
    transform: scale(0.88);
  }
  .vote.on {
    background: linear-gradient(135deg, #ffe08a, var(--gold));
    animation: pop-in 0.5s var(--spring);
  }
  .info {
    display: flex;
    align-items: center;
    padding: 12px 14px 8px;
  }
  .info h3 {
    font-size: 20px;
  }
  .meta {
    margin-top: 3px;
    font-size: 14px;
    color: var(--muted);
  }
  .meta b {
    color: var(--ink);
  }
  .dotsep {
    margin: 0 4px;
  }
  .timeline {
    display: flex;
    gap: 3px;
    padding: 0 14px 14px;
  }
  .timeline i {
    flex: 1;
    height: 6px;
    border-radius: 4px;
    opacity: 0.85;
  }
  .hero {
    position: relative;
    height: 230px;
    background-size: cover;
    background-position: center;
    display: flex;
    align-items: flex-end;
    padding: 18px 20px;
  }
  .hshade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgba(0, 0, 0, 0.55), transparent 60%);
  }
  .hero h2 {
    position: relative;
    color: #fff;
    font-size: 30px;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.4);
  }
  .dstats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .st {
    padding: 12px;
    border-radius: 18px;
    background: var(--card);
    border: 1px solid var(--line);
    display: flex;
    flex-direction: column;
  }
  .st small {
    font-size: 11px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  .st b {
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 600;
    margin: 2px 0;
  }
  .st span {
    font-size: 12px;
    color: var(--muted);
  }
  .sh {
    font-size: 18px;
    margin: 22px 0 10px;
  }
  .shelter {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .hours {
    display: flex;
    gap: 4px;
    align-items: flex-end;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .hh {
    flex: 1 0 22px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .hh i {
    width: 100%;
    border-radius: 6px;
    animation: grow 0.8s var(--spring) both;
    transform-origin: bottom;
  }
  @keyframes grow {
    from {
      transform: scaleY(0);
    }
  }
  .hh span {
    font-size: 10.5px;
    font-weight: 800;
    color: var(--muted);
  }
  .nav {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .nav a {
    text-decoration: none;
  }
</style>
