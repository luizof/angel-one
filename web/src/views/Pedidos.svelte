<script lang="ts">
  import { app } from '../lib/store.svelte'
  import { api } from '../lib/api'
  import type { Order, OrderStatus } from '../lib/types'
  import { timeAgo, hhmm, isStandalone } from '../lib/util'
  import { soundEnabled, setSoundEnabled, chime, pop, tada } from '../lib/sound'
  import { burstFrom } from '../lib/fx'
  import { flip } from 'svelte/animate'
  import { fly, scale } from 'svelte/transition'

  let sound = $state(soundEnabled())
  let openRecipe = $state<Record<number, boolean>>({})
  let now = $state(Date.now())
  $effect(() => {
    const t = setInterval(() => now = Date.now(), 30_000)
    return () => clearInterval(t)
  })

  const groups = $derived([
    { id: 'recebido', label: 'Novos', emoji: '🛎️', list: app.orders.filter((o) => o.status === 'recebido') },
    { id: 'preparando', label: 'Preparando', emoji: '🍸', list: app.orders.filter((o) => o.status === 'preparando') },
    { id: 'pronto', label: 'Prontos para entregar', emoji: '🪽', list: app.orders.filter((o) => o.status === 'pronto') },
  ])
  const done = $derived(app.orders.filter((o) => o.status === 'entregue' || o.status === 'cancelado').slice(0, 12))
  const NEXT: Partial<Record<OrderStatus, { to: OrderStatus; label: string }>> = {
    recebido: { to: 'preparando', label: 'Começar a preparar' },
    preparando: { to: 'pronto', label: 'Está pronto!' },
    pronto: { to: 'entregue', label: 'Entregue ✓' },
  }

  async function advance(o: Order, to: OrderStatus, e: Event) {
    const el = e.currentTarget as Element
    try {
      await api(`/api/orders/${o.id}`, { method: 'PATCH', body: { status: to } })
      if (to === 'pronto') {
        tada()
        burstFrom(el, 'confetti', 50)
      } else if (to === 'entregue') {
        burstFrom(el, 'feathers', 10)
        pop()
      } else pop()
      app.loadOrders()
    } catch (err) {
      app.fail(err)
    }
  }
  async function cancel(o: Order) {
    if (!confirm(`Cancelar ${o.item_name} de ${o.for_name}?`)) return
    try {
      await api(`/api/orders/${o.id}`, { method: 'PATCH', body: { status: 'cancelado' } })
      app.loadOrders()
    } catch (err) {
      app.fail(err)
    }
  }

  function toggleSound() {
    sound = !sound
    setSoundEnabled(sound)
    if (sound) chime()
  }
</script>

<div class="page">
  <div class="page-title">
    <div class="bell"><span>🛎️</span></div>
    <div class="grow">
      <h1>Comandas</h1>
      <div class="sub">Pedidos chegam aqui na hora, com a receita.</div>
    </div>
  </div>

  <div class="toolbar">
    <button class="chip" class:on={sound} onclick={toggleSound}>{sound ? '🔔 Som ligado' : '🔕 Som desligado'}</button>
    {#if app.pushState === 'subscribed'}
      <span class="chip">📲 Avisos ativos</span>
    {:else if app.pushState === 'unsupported' || !isStandalone()}
      <span class="chip muted" title="No iPhone, instale o app na tela inicial para receber avisos">📲 Instale na tela inicial p/ avisos</span>
    {:else if app.pushState === 'denied'}
      <span class="chip muted">📵 Avisos bloqueados</span>
    {:else}
      <button class="chip" onclick={() => app.enablePush()}>📲 Ativar avisos</button>
    {/if}
  </div>

  {#each groups as g (g.id)}
    {#if g.list.length || g.id === 'recebido'}
      <div class="section-h"><h2>{g.emoji} {g.label}</h2><span class="count">{g.list.length}</span></div>
      {#if !g.list.length}
        <div class="empty card small"><div class="art">🏖️</div><p>Nenhum pedido novo. Aproveite o mar!</p></div>
      {/if}
      <div class="tickets">
        {#each g.list as o (o.id)}
          <div class="ticket s-{o.status}" animate:flip={{ duration: 400 }} in:fly={{ y: -30, duration: 500 }} out:scale={{ duration: 250, start: 0.9 }}>
            <div class="t-top">
              {#if o.item_photo}<img src={o.item_photo} alt="" />{:else}<span class="noph">🍹</span>{/if}
              <div class="grow">
                <div class="t-item">{o.item_name}</div>
                <div class="t-for">para <b>{o.for_name}</b></div>
                <div class="t-time">{hhmm(o.created_at)} · {timeAgo(o.created_at, now)}{#if o.handled_by} · {o.handled_by}{/if}</div>
              </div>
            </div>
            {#if o.notes}<div class="t-notes">✍️ {o.notes}</div>{/if}
            {#if o.recipe}
              {@const showRec = !!openRecipe[o.id] !== (o.status === 'preparando')}
              <button class="t-rec-t" onclick={() => (openRecipe[o.id] = !openRecipe[o.id])}>📜 {showRec ? 'Esconder receita' : 'Ver receita'}</button>
              {#if showRec}
                <pre class="t-rec">{o.recipe}</pre>
              {/if}
            {/if}
            <div class="t-acts">
              <button class="btn sm ghost" onclick={() => cancel(o)}>Cancelar</button>
              {#if NEXT[o.status]}
                <button class="btn sm grow {o.status === 'preparando' ? 'gold' : ''}" onclick={(e) => advance(o, NEXT[o.status]!.to, e)}>{NEXT[o.status]!.label}</button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/each}

  {#if done.length}
    <div class="section-h"><h2>Histórico</h2></div>
    <div class="card hist">
      {#each done as o (o.id)}
        <div class="h-row">
          <span>{o.status === 'entregue' ? '✅' : '❌'}</span>
          <span class="grow"><b>{o.item_name}</b> · {o.for_name}</span>
          <span class="small muted">{hhmm(o.updated_at)}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .bell span {
    display: inline-block;
    font-size: 40px;
    animation: wiggle 2.8s ease-in-out infinite;
    transform-origin: 50% 10%;
  }
  .toolbar {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .count {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 20px;
    color: var(--accent);
  }
  .tickets {
    display: grid;
    gap: 14px;
  }
  .ticket {
    position: relative;
    padding: 16px 16px 14px;
    background: var(--card-solid);
    border-radius: 18px 18px 6px 6px;
    box-shadow: var(--shadow);
    border-left: 6px solid var(--accent);
  }
  /* borda serrilhada de comanda */
  .ticket::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    bottom: -8px;
    height: 10px;
    background: radial-gradient(circle at 6px -2px, transparent 7px, var(--card-solid) 7.5px) repeat-x;
    background-size: 12px 10px;
  }
  .s-recebido {
    border-left-color: var(--bad);
    animation: newpulse 2s ease-in-out 3;
  }
  @keyframes newpulse {
    50% {
      box-shadow: 0 0 0 6px color-mix(in srgb, var(--bad) 25%, transparent), var(--shadow);
    }
  }
  .s-preparando {
    border-left-color: var(--warn);
  }
  .s-pronto {
    border-left-color: var(--good);
  }
  .t-top {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .t-top img,
  .noph {
    width: 64px;
    height: 64px;
    border-radius: 16px;
    object-fit: cover;
    display: grid;
    place-items: center;
    font-size: 30px;
    background: var(--card-2);
  }
  .t-item {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 20px;
    line-height: 1.1;
  }
  .t-for {
    font-size: 16px;
    margin-top: 2px;
  }
  .t-for b {
    font-size: 18px;
    color: var(--accent);
  }
  .t-time {
    font-size: 12.5px;
    color: var(--muted);
    margin-top: 2px;
  }
  .t-notes {
    margin-top: 10px;
    padding: 8px 12px;
    border-radius: 12px;
    background: color-mix(in srgb, var(--gold) 22%, transparent);
    font-weight: 800;
  }
  .t-rec-t {
    margin-top: 10px;
    font-weight: 800;
    font-size: 14px;
    color: var(--accent);
  }
  .t-rec {
    white-space: pre-wrap;
    font-family: var(--font-body);
    font-size: 14.5px;
    margin: 8px 0 0;
    padding: 12px;
    border-radius: 12px;
    background: var(--card);
    border: 1px dashed var(--line);
  }
  .t-acts {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }
  .hist {
    padding: 6px 14px;
  }
  .h-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 0;
    border-bottom: 1px solid var(--line);
    font-size: 14px;
  }
  .h-row:last-child {
    border-bottom: 0;
  }
</style>
