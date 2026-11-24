<script lang="ts">
  // Conferência do inventário físico, item por item, em tela cheia.
  import { onMount } from 'svelte'
  import { app } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { api } from '../lib/api'
  import QtyEditor from '../components/QtyEditor.svelte'
  import Icon from '../components/Icon.svelte'
  import type { Item } from '../lib/types'
  import { qtyLabel, contentLabel } from '../lib/inv'
  import { fmt2 } from '../lib/util'
  import { burst } from '../lib/fx'
  import { tada, pop } from '../lib/sound'
  import { fly } from 'svelte/transition'

  // ordem fixa no início da conferência: por grupo e nome
  let order = $state<Item[]>([])
  let started = false

  let idx = $state(0)
  let dir = $state(1)
  let qty = $state(0)
  let saving = $state(false)
  let changes = $state<{ name: string; before: number; after: number }[]>([])
  let finished = $state(false)
  onMount(() => window.scrollTo(0, 0))

  $effect(() => {
    if (started || !app.loaded) return
    started = true
    const tagName = (i: Item) => app.inventory.tags.find((t) => i.tag_ids.includes(t.id))?.name ?? '~'
    order = [...app.inventory.items].sort((a, b) => tagName(a).localeCompare(tagName(b)) || a.name.localeCompare(b.name))
    qty = order[0]?.quantity ?? 0
    finished = order.length === 0
  })

  const cur = $derived(order[idx])
  const fresh = $derived(cur ? (app.inventory.items.find((i) => i.id === cur.id) ?? cur) : null)
  const tags = $derived(fresh ? app.inventory.tags.filter((t) => fresh!.tag_ids.includes(t.id)) : [])

  function go(n: number) {
    dir = n > idx ? 1 : -1
    if (n >= order.length) {
      finished = true
      tada()
      burst('confetti', innerWidth / 2, innerHeight * 0.3, 110)
      return
    }
    idx = Math.max(0, n)
    const it = app.inventory.items.find((i) => i.id === order[idx].id) ?? order[idx]
    qty = it.quantity
  }

  async function confirm() {
    if (!fresh) return
    const before = fresh.quantity
    if (Math.abs(qty - before) > 1e-9) {
      saving = true
      try {
        await api(`/api/items/${fresh.id}/adjust`, { body: { quantity: qty, reason: 'conferencia' } })
        changes = [...changes, { name: fresh.name, before, after: qty }]
        app.loadInventory()
      } catch (e) {
        app.fail(e)
        saving = false
        return
      }
      saving = false
    }
    pop()
    go(idx + 1)
  }
</script>

<div class="conf">
  <header>
    <button class="icon-btn" onclick={() => router.go('/despensa', true)} aria-label="Sair"><Icon name="close" /></button>
    <div class="grow center">
      <div class="ttl">Conferência</div>
      {#if !finished}<div class="small muted">{idx + 1} de {order.length}</div>{/if}
    </div>
    <span style="width:42px"></span>
  </header>
  <div class="prog"><i style="width:{finished ? 100 : (idx / Math.max(1, order.length)) * 100}%"></i></div>

  {#if finished}
    <div class="done" in:fly={{ y: 30, duration: 500 }}>
      <div class="trophy">🏆</div>
      <h2>Despensa conferida!</h2>
      <p class="muted">{changes.length ? `${changes.length} ${changes.length === 1 ? 'item ajustado' : 'itens ajustados'}` : 'Tudo batia certinho.'}</p>
      {#if changes.length}
        <div class="card pad chg">
          {#each changes as c}
            <div class="crow"><span class="grow">{c.name}</span><span class="muted">{fmt2(c.before)}</span><span>→</span><b>{fmt2(c.after)}</b></div>
          {/each}
        </div>
      {/if}
      <button class="btn block" style="margin-top:20px" onclick={() => router.go('/despensa/compras', true)}>Ver lista de compras</button>
      <button class="btn ghost block" style="margin-top:10px" onclick={() => router.go('/despensa', true)}>Voltar à despensa</button>
    </div>
  {:else if fresh}
    {#key idx}
      <div class="card-wrap" in:fly={{ x: 60 * dir, duration: 380 }}>
        <div class="photo">
          {#if fresh.photo}<img src={fresh.photo} alt="" />{:else}<span>📦</span>{/if}
        </div>
        <h2>{fresh.name}</h2>
        <div class="sub">
          {#each tags as t}<span class="pill">{t.emoji} {t.name}</span>{/each}
          {#if contentLabel(fresh)}<span class="pill">{contentLabel(fresh)}</span>{/if}
        </div>
        <p class="was small muted">No sistema: {qtyLabel(fresh)}</p>
        <QtyEditor bind:value={qty} fractional={fresh.fractional} unit={fresh.count_unit} />
      </div>
    {/key}
  {/if}

  {#if !finished}
    <footer>
      <button class="btn ghost" onclick={() => go(idx - 1)} disabled={idx === 0}><Icon name="back" size={18} /></button>
      <button class="btn ghost" onclick={() => go(idx + 1)}>Pular</button>
      <button class="btn grow" onclick={confirm} disabled={saving}>
        {fresh && Math.abs(qty - fresh.quantity) > 1e-9 ? 'Salvar e próximo' : 'Confere ✓'}
      </button>
    </footer>
  {/if}
</div>

<style>
  .conf {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    padding: 0 18px calc(var(--safe-bottom) + 100px);
    max-width: 560px;
    margin: 0 auto;
  }
  header {
    position: sticky;
    top: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    min-height: calc(var(--safe-top) + 60px);
    padding-top: calc(var(--safe-top) + 8px);
    background: var(--bg);
  }
  .ttl {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 20px;
  }
  .prog {
    height: 6px;
    border-radius: 6px;
    background: var(--line);
    margin: 12px 0 18px;
    overflow: hidden;
  }
  .prog i {
    display: block;
    height: 100%;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--accent), var(--accent2));
    transition: width 0.5s var(--spring);
  }
  .card-wrap {
    text-align: center;
  }
  .photo {
    width: 180px;
    height: 180px;
    margin: 0 auto;
    border-radius: 30px;
    background: #fff;
    box-shadow: var(--shadow);
    overflow: hidden;
    display: grid;
    place-items: center;
    font-size: 70px;
  }
  .photo img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  h2 {
    font-size: 26px;
    margin-top: 16px;
  }
  .sub {
    display: flex;
    gap: 6px;
    justify-content: center;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .was {
    margin: 10px 0 16px;
  }
  footer {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    gap: 8px;
    padding: 12px 18px calc(var(--safe-bottom) + 14px);
    background: color-mix(in srgb, var(--bg) 90%, transparent);
    -webkit-backdrop-filter: blur(16px);
    backdrop-filter: blur(16px);
    border-top: 1px solid var(--line);
    z-index: 50;
  }
  .done {
    text-align: center;
    padding-top: 30px;
  }
  .trophy {
    font-size: 80px;
    animation: bob 2.4s ease-in-out infinite;
  }
  .chg {
    margin-top: 16px;
    text-align: left;
  }
  .crow {
    display: flex;
    gap: 8px;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
</style>
