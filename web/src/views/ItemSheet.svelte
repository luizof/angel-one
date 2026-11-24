<script lang="ts">
  import Sheet from '../components/Sheet.svelte'
  import QtyEditor from '../components/QtyEditor.svelte'
  import Icon from '../components/Icon.svelte'
  import { app } from '../lib/store.svelte'
  import { api } from '../lib/api'
  import type { Item, Movement } from '../lib/types'
  import { qtyLabel, contentLabel, itemLow, unitLabel } from '../lib/inv'
  import { fmt2, timeAgo } from '../lib/util'
  import { pop } from '../lib/sound'

  let { open = $bindable(false), item = null as Item | null, onedit }: { open?: boolean; item?: Item | null; onedit?: (i: Item) => void } = $props()

  let qty = $state(0)
  let saving = $state(false)
  let history = $state<Movement[] | null>(null)
  let showHist = $state(false)
  let lastId = -1

  $effect(() => {
    if (open && item && item.id !== lastId) {
      lastId = item.id
      qty = item.quantity
      history = null
      showHist = false
    }
    if (!open) lastId = -1
  })

  const tags = $derived(item ? app.inventory.tags.filter((t) => item!.tag_ids.includes(t.id)) : [])
  const changed = $derived(item != null && Math.abs(qty - item.quantity) > 1e-9)

  async function save() {
    if (!item) return
    saving = true
    try {
      await api(`/api/items/${item.id}/adjust`, { body: { quantity: qty, reason: 'ajuste' } })
      pop()
      app.toast('Estoque atualizado', 'ok', '📦')
      app.loadInventory()
      history = null
    } catch (e) {
      app.fail(e)
    } finally {
      saving = false
    }
  }

  async function loadHist() {
    showHist = !showHist
    if (!showHist || !item) return
    try {
      history = await api<Movement[]>(`/api/items/${item.id}/history`)
    } catch (e) {
      app.fail(e)
    }
  }
  const REASON: Record<string, string> = { conferencia: 'Conferência', compra: 'Compra', ajuste: 'Ajuste' }
</script>

<Sheet bind:open>
  {#snippet hero()}
    {#if item}
      <div class="hero">
        {#if item.photo}<img src={item.photo} alt="" />{:else}<span class="ph">📦</span>{/if}
      </div>
    {/if}
  {/snippet}
  {#if item}
    <h2 class="t">{item.name}</h2>
    <div class="meta">
      {#each tags as t (t.id)}<span class="pill tag">{t.emoji} {t.name}</span>{/each}
      {#if contentLabel(item)}<span class="pill">{contentLabel(item)} cada</span>{/if}
    </div>

    <div class="now card pad" class:low={itemLow(item)}>
      <div>
        <div class="label">Em estoque</div>
        <div class="big">{qtyLabel(item)}</div>
        {#if item.min_qty != null}
          <div class="small muted">mínimo: {fmt2(item.min_qty)} {unitLabel(item.count_unit, item.min_qty)}</div>
        {/if}
      </div>
      {#if itemLow(item)}<span class="buy">🛒 Comprar</span>{/if}
    </div>
    {#each tags as t (t.id)}
      <p class="small muted" style="margin:8px 4px 0">{t.emoji} Total de {t.name}: <b>{fmt2(t.total)} {t.unit}</b>{#if t.min_qty != null} (mín. {fmt2(t.min_qty)} {t.unit}){/if}</p>
    {/each}

    {#if app.isCrew}
      <h3 class="sh">Atualizar quantidade</h3>
      <QtyEditor bind:value={qty} fractional={item.fractional} unit={item.count_unit} />
      <div class="row" style="margin-top:14px">
        <button class="btn ghost" onclick={() => onedit?.(item!)}><Icon name="edit" size={18} /> Editar</button>
        <button class="btn grow" onclick={save} disabled={!changed || saving}>{saving ? 'Salvando…' : 'Salvar quantidade'}</button>
      </div>
      <button class="hist-t" onclick={loadHist}><Icon name="history" size={16} /> {showHist ? 'Esconder' : 'Ver'} histórico</button>
      {#if showHist}
        <div class="hist">
          {#if history == null}
            <div class="skeleton" style="height:60px"></div>
          {:else if !history.length}
            <p class="small muted">Sem movimentações ainda.</p>
          {:else}
            {#each history as h}
              <div class="hrow">
                <span class="r">{REASON[h.reason] ?? h.reason}</span>
                <span class="grow">{fmt2(h.before)} → <b>{fmt2(h.after)}</b></span>
                <span class="small muted">{h.by ?? ''} · {timeAgo(h.at)}</span>
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    {:else if item.updated_at}
      <p class="small muted" style="margin-top:12px">Atualizado {timeAgo(item.updated_at)}{item.updated_by ? ` por ${item.updated_by}` : ''}.</p>
    {/if}
  {/if}
</Sheet>

<style>
  .hero {
    height: 250px;
    background: #fff;
    display: grid;
    place-items: center;
  }
  .hero img {
    height: 100%;
    width: 100%;
    object-fit: contain;
  }
  .ph {
    font-size: 80px;
  }
  .t {
    font-size: 26px;
    margin-top: 8px;
  }
  .meta {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .tag {
    color: var(--accent);
  }
  .now {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 16px;
  }
  .now.low {
    border-color: color-mix(in srgb, var(--bad) 55%, transparent);
  }
  .big {
    font-family: var(--font-display);
    font-size: 30px;
    font-weight: 600;
  }
  .buy {
    font-weight: 900;
    color: var(--bad);
    animation: wiggle 2s ease-in-out infinite;
  }
  .sh {
    font-size: 18px;
    margin: 22px 0 10px;
  }
  .hist-t {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 16px;
    font-weight: 800;
    color: var(--muted);
  }
  .hist {
    margin-top: 8px;
  }
  .hrow {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
    font-size: 14px;
  }
  .r {
    font-weight: 800;
    color: var(--accent);
    min-width: 88px;
  }
</style>
