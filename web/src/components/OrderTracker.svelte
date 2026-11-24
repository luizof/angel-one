<script lang="ts">
  // Acompanhamento dos pedidos deste aparelho: recebido → preparando → pronto.
  import { app } from '../lib/store.svelte'
  import { api } from '../lib/api'
  import { timeAgo } from '../lib/util'
  import type { Order } from '../lib/types'
  import { fly } from 'svelte/transition'

  const STEPS = ['recebido', 'preparando', 'pronto'] as const
  const ICON = { recebido: '🛎️', preparando: '🍸', pronto: '🪽' }
  const LBL = { recebido: 'Recebido', preparando: 'Preparando', pronto: 'Pronto!' }

  const mine = $derived(
    app.isCrew ? [] : app.orders.filter((o) => o.status === 'recebido' || o.status === 'preparando' || (o.status === 'pronto' && Date.now() - new Date(o.updated_at).getTime() < 30 * 60_000)),
  )

  async function cancel(o: Order) {
    try {
      await api(`/api/orders/${o.id}`, { method: 'PATCH', body: { status: 'cancelado' } })
      app.toast('Pedido cancelado', 'info', '🙅')
      app.loadOrders()
    } catch (e) {
      app.fail(e)
    }
  }
</script>

{#if mine.length}
  <div class="tracker">
    {#each mine as o (o.id)}
      {@const idx = STEPS.indexOf(o.status as any)}
      <div class="ord card" class:ready={o.status === 'pronto'} in:fly={{ y: 20, duration: 400 }}>
        <div class="row">
          {#if o.item_photo}<img class="th" src={o.item_photo} alt="" />{:else}<span class="th e">🍹</span>{/if}
          <div class="grow">
            <div class="nm">{o.item_name}</div>
            <div class="small muted">para <b>{o.for_name}</b> · {timeAgo(o.created_at)}</div>
          </div>
          <span class="big-ico" class:shake={o.status === 'preparando'} class:bounce={o.status === 'pronto'}>{ICON[o.status as keyof typeof ICON]}</span>
        </div>
        <div class="steps">
          {#each STEPS as s, i}
            <div class="st" class:done={i <= idx} class:cur={i === idx}>
              <span class="dot"></span>
              <span class="l">{LBL[s]}</span>
            </div>
            {#if i < STEPS.length - 1}<div class="line" class:done={i < idx}><i></i></div>{/if}
          {/each}
        </div>
        {#if o.status === 'recebido'}
          <button class="cancel" onclick={() => cancel(o)}>cancelar</button>
        {/if}
        {#if o.status === 'pronto'}
          <div class="ready-msg">Seu pedido está pronto. Pode buscar! 🥂</div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .tracker {
    display: grid;
    gap: 10px;
  }
  .ord {
    padding: 14px;
    position: relative;
  }
  .ord.ready {
    border-color: color-mix(in srgb, var(--gold) 70%, transparent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--gold) 25%, transparent), var(--shadow);
  }
  .th {
    width: 48px;
    height: 48px;
    border-radius: 14px;
    object-fit: cover;
  }
  .th.e {
    display: grid;
    place-items: center;
    font-size: 26px;
    background: var(--card-2);
  }
  .nm {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 18px;
  }
  .big-ico {
    font-size: 30px;
  }
  .shake {
    animation: shake 0.6s ease-in-out infinite;
    display: inline-block;
  }
  @keyframes shake {
    0%,
    100% {
      transform: rotate(-12deg) translateY(0);
    }
    50% {
      transform: rotate(12deg) translateY(-4px);
    }
  }
  .bounce {
    display: inline-block;
    animation: bob 1.2s ease-in-out infinite;
  }
  .steps {
    display: flex;
    align-items: center;
    margin-top: 14px;
  }
  .st {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    min-width: 64px;
  }
  .dot {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--line);
    transition: background 0.4s, transform 0.4s var(--spring);
  }
  .st.done .dot {
    background: var(--accent);
  }
  .st.cur .dot {
    transform: scale(1.35);
    box-shadow: 0 0 0 5px color-mix(in srgb, var(--accent) 22%, transparent);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      box-shadow: 0 0 0 9px color-mix(in srgb, var(--accent) 8%, transparent);
    }
  }
  .l {
    font-size: 11.5px;
    font-weight: 800;
    color: var(--muted);
  }
  .st.done .l {
    color: var(--ink);
  }
  .line {
    flex: 1;
    height: 4px;
    border-radius: 4px;
    background: var(--line);
    margin-bottom: 18px;
    overflow: hidden;
  }
  .line i {
    display: block;
    height: 100%;
    width: 0;
    background: var(--accent);
    transition: width 0.8s var(--ease-out);
  }
  .line.done i {
    width: 100%;
  }
  .cancel {
    display: block;
    margin: 6px 0 0 auto;
    font-size: 12px;
    font-weight: 800;
    color: var(--muted);
    text-decoration: underline;
  }
  .ord .row {
    padding-right: 6px;
  }
  .ready-msg {
    margin-top: 10px;
    text-align: center;
    font-weight: 800;
    color: var(--ink);
  }
</style>
