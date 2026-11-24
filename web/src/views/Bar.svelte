<script lang="ts">
  import { app } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { api } from '../lib/api'
  import type { Category, MenuItem } from '../lib/types'
  import { burst } from '../lib/fx'
  import { tada, pop } from '../lib/sound'
  import { isStandalone } from '../lib/util'
  import Sheet from '../components/Sheet.svelte'
  import Icon from '../components/Icon.svelte'
  import OrderTracker from '../components/OrderTracker.svelte'
  import MenuForm from './MenuForm.svelte'

  const CATS: { id: Category; label: string; emoji: string }[] = [
    { id: 'drink', label: 'Drinks', emoji: '🍹' },
    { id: 'petisco', label: 'Petiscos', emoji: '🍤' },
    { id: 'bebida', label: 'Bebidas', emoji: '🥤' },
  ]
  let cat = $state<Category>((sessionStorage.getItem('ao.barCat') as Category) || 'drink')
  $effect(() => sessionStorage.setItem('ao.barCat', cat))
  const catIdx = $derived(CATS.findIndex((c) => c.id === cat))

  const items = $derived(
    app.menu
      .filter((m) => m.category === cat)
      .sort((a, b) => Number(b.available) - Number(a.available) || a.sort - b.sort || a.name.localeCompare(b.name)),
  )

  const openId = $derived(Number(router.path.split('/')[2]) || null)
  const open = $derived(openId != null ? app.menu.find((m) => m.id === openId) : undefined)
  let sheetOpen = $state(false)
  $effect(() => {
    sheetOpen = !!open
    if (open) {
      notes = ''
      forName = localStorage.getItem('ao.lastName') ?? ''
      showRecipe = false
    }
  })

  let forName = $state('')
  let notes = $state('')
  let sending = $state(false)
  let showRecipe = $state(false)
  let formOpen = $state(false)
  let editing = $state<MenuItem | null>(null)

  function openItem(m: MenuItem) {
    pop()
    router.go(`/bar/${m.id}`)
  }
  function closeItem() {
    router.go('/bar', true)
  }

  async function order() {
    if (!open || !forName.trim()) return
    sending = true
    try {
      await api('/api/orders', { body: { menu_item_id: open.id, for_name: forName.trim(), notes: notes.trim() } })
      localStorage.setItem('ao.lastName', forName.trim())
      tada()
      burst('confetti', innerWidth / 2, innerHeight * 0.55)
      app.toast(`Pedido enviado! ${open.name} para ${forName.trim()}`, 'ok', '🍹')
      sheetOpen = false
      closeItem()
      app.loadOrders()
      if (isStandalone() && app.pushState === 'default') {
        setTimeout(() => app.toast('Dica: ative os avisos em Bar → "Avise quando ficar pronto"', 'info', '🔔'), 1600)
      }
    } catch (e) {
      app.fail(e)
    } finally {
      sending = false
    }
  }

  async function toggleAvail(m: MenuItem, e: Event) {
    e.stopPropagation()
    try {
      await api(`/api/menu/${m.id}`, { method: 'PATCH', body: { available: !m.available } })
      pop()
      app.loadMenu()
    } catch (err) {
      app.fail(err)
    }
  }

  function newItem() {
    editing = null
    formOpen = true
  }
  function editItem(m: MenuItem) {
    editing = m
    sheetOpen = false
    closeItem()
    setTimeout(() => (formOpen = true), 250)
  }

  const myActive = $derived(!app.isCrew && app.orders.some((o) => o.status === 'recebido' || o.status === 'preparando'))
</script>

<div class="page bar">
  <div class="page-title">
    <div class="glass-ico">
      <span class="liquid"></span>
      <span class="bub b1"></span><span class="bub b2"></span><span class="bub b3"></span>
    </div>
    <div class="grow">
      <h1>Bar do Anjo</h1>
      <div class="sub">Peça e a tripulação prepara. Sem conta no final 😉</div>
    </div>
    {#if app.isCrew}
      <button class="icon-btn add" onclick={newItem} aria-label="Novo item"><Icon name="plus" /></button>
    {/if}
  </div>

  <OrderTracker />
  {#if myActive && app.pushState === 'default' && isStandalone()}
    <button class="btn soft block" style="margin-top:10px" onclick={() => app.enablePush()}>🔔 Avise quando ficar pronto</button>
  {/if}

  <div class="seg" style="--i:{catIdx}">
    <span class="ind"></span>
    {#each CATS as c}
      <button class:on={cat === c.id} onclick={() => { pop(); cat = c.id }}>
        <span class="e">{c.emoji}</span> {c.label}
      </button>
    {/each}
  </div>

  {#key cat}
    <div class="grid stagger">
      {#each items as m (m.id)}
        <div class="it" class:off={!m.available} role="button" tabindex="0" onclick={() => m.available || app.isCrew ? openItem(m) : null} onkeydown={(e) => e.key === 'Enter' && openItem(m)}>
          <div class="ph" style="background-image:url({m.photo ?? ''})">
            {#if !m.photo}<span class="noph">{CATS.find((c) => c.id === m.category)?.emoji}</span>{/if}
            {#if !m.available}<span class="stamp">Esgotado</span>{/if}
          </div>
          <div class="nm">{m.name}</div>
          {#if m.description}<div class="ds">{m.description}</div>{/if}
          {#if app.isCrew}
            <button class="avail" class:on={m.available} onclick={(e) => toggleAvail(m, e)}>
              <span class="switch" class:on={m.available}></span>
              <span>{m.available ? 'Disponível' : 'Indisponível'}</span>
            </button>
          {/if}
        </div>
      {/each}
    </div>
    {#if !items.length}
      <div class="empty card">
        <div class="art">{CATS[catIdx].emoji}</div>
        <p>Nada em {CATS[catIdx].label.toLowerCase()} por enquanto.</p>
        {#if app.isCrew}<button class="btn" style="margin-top:14px" onclick={newItem}>Cadastrar com IA ✨</button>{/if}
      </div>
    {/if}
  {/key}
</div>

<Sheet bind:open={sheetOpen} onclose={closeItem}>
  {#snippet hero()}
    {#if open}
      <div class="ohero" style="background-image:url({open.photo ?? ''})">
        {#if !open.photo}<span class="noph big">🍹</span>{/if}
      </div>
    {/if}
  {/snippet}
  {#if open}
    <h2 class="otitle">{open.name}</h2>
    {#if open.description}<p class="muted" style="margin-top:6px">{open.description}</p>{/if}

    {#if app.isCrew && open.recipe}
      <button class="recipe-t" onclick={() => (showRecipe = !showRecipe)}>📜 {showRecipe ? 'Esconder' : 'Ver'} receita</button>
      {#if showRecipe}<pre class="recipe">{open.recipe}</pre>{/if}
    {/if}

    {#if open.available}
      <label class="field" style="margin-top:18px">
        <span>Para quem é?</span>
        <input class="input big" bind:value={forName} placeholder="Primeiro nome" maxlength="24" autocomplete="given-name" enterkeyhint="send" />
      </label>
      <label class="field">
        <span>Observação (opcional)</span>
        <input class="input" bind:value={notes} placeholder="Ex.: pouco gelo, sem açúcar…" maxlength="120" />
      </label>
    {:else}
      <div class="card pad center" style="margin-top:18px">Este item está indisponível agora. 😢</div>
    {/if}

    {#if app.isCrew}
      <button class="btn soft block" style="margin-top:8px" onclick={() => editItem(open)}><Icon name="edit" size={18} /> Editar item</button>
    {/if}
  {/if}
  {#snippet footer()}
    {#if open?.available}
      <button class="btn block gold" onclick={order} disabled={sending || !forName.trim()}>
        {sending ? 'Enviando…' : forName.trim() ? `Pedir para ${forName.trim()} 🥂` : 'Diga para quem é'}
      </button>
    {/if}
  {/snippet}
</Sheet>

{#if app.isCrew}
  <MenuForm bind:open={formOpen} item={editing} defaultCategory={cat} />
{/if}

<style>
  .glass-ico {
    position: relative;
    width: 46px;
    height: 54px;
    border-radius: 6px 6px 18px 18px;
    border: 3px solid var(--ink);
    border-top-width: 0;
    overflow: hidden;
    flex: none;
    opacity: 0.85;
  }
  .liquid {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 65%;
    background: linear-gradient(180deg, var(--accent2), var(--accent));
    animation: liquid 3s ease-in-out infinite;
  }
  @keyframes liquid {
    50% {
      height: 72%;
    }
  }
  .bub {
    position: absolute;
    bottom: 4px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    border: 1.5px solid rgba(255, 255, 255, 0.85);
    animation: bubble 2.4s ease-in infinite;
  }
  .b1 {
    left: 8px;
  }
  .b2 {
    left: 20px;
    animation-delay: 0.8s;
  }
  .b3 {
    left: 30px;
    animation-delay: 1.5s;
  }
  @keyframes bubble {
    from {
      transform: translateY(0);
      opacity: 1;
    }
    to {
      transform: translateY(-34px);
      opacity: 0;
    }
  }
  .add {
    background: var(--accent);
    color: var(--accent-ink);
    border: 0;
    box-shadow: 0 8px 18px -8px var(--accent);
  }
  .seg {
    position: relative;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    padding: 5px;
    margin: 18px 0 16px;
    border-radius: 20px;
    background: var(--card);
    border: 1px solid var(--line);
  }
  .seg .ind {
    position: absolute;
    top: 5px;
    bottom: 5px;
    left: 5px;
    width: calc((100% - 10px) / 3);
    border-radius: 15px;
    background: var(--accent);
    box-shadow: 0 8px 16px -8px var(--accent);
    transform: translateX(calc(100% * var(--i)));
    transition: transform 0.5s var(--spring);
  }
  .seg button {
    position: relative;
    height: 44px;
    font-weight: 800;
    font-size: 15px;
    color: var(--muted);
    transition: color 0.3s;
  }
  .seg button.on {
    color: var(--accent-ink);
  }
  .seg .e {
    display: inline-block;
    transition: transform 0.4s var(--spring);
  }
  .seg button.on .e {
    transform: scale(1.25) rotate(-8deg);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  .it {
    cursor: pointer;
    transition: transform 0.3s var(--spring);
  }
  .it:active {
    transform: scale(0.96) rotate(-0.6deg);
  }
  .it .ph {
    position: relative;
    aspect-ratio: 1;
    border-radius: 24px;
    background: var(--card-2) center/cover;
    box-shadow: var(--shadow);
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .noph {
    font-size: 48px;
  }
  .noph.big {
    font-size: 90px;
  }
  .it.off .ph {
    filter: grayscale(0.9) brightness(0.9);
  }
  .stamp {
    position: absolute;
    padding: 4px 12px;
    border: 3px solid var(--bad);
    color: var(--bad);
    background: rgba(255, 255, 255, 0.85);
    border-radius: 10px;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 17px;
    transform: rotate(-14deg);
    text-transform: uppercase;
  }
  .nm {
    margin-top: 9px;
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 17px;
    line-height: 1.15;
  }
  .ds {
    font-size: 13px;
    color: var(--muted);
    margin-top: 3px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .avail {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    font-size: 12.5px;
    font-weight: 800;
    color: var(--muted);
  }
  .avail .switch {
    transform: scale(0.8);
    transform-origin: left center;
    margin-right: -8px;
  }
  .ohero {
    height: 300px;
    background: var(--card-2) center/cover;
    display: grid;
    place-items: center;
  }
  .otitle {
    font-size: 28px;
    margin-top: 6px;
  }
  .input.big {
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 600;
  }
  .recipe-t {
    margin-top: 14px;
    font-weight: 800;
    color: var(--accent);
  }
  .recipe {
    white-space: pre-wrap;
    font-family: var(--font-body);
    font-size: 15px;
    padding: 14px;
    border-radius: 16px;
    background: var(--card);
    border: 1px solid var(--line);
    margin: 10px 0 0;
  }
</style>
