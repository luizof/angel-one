<script lang="ts">
  import Sheet from '../components/Sheet.svelte'
  import QtyEditor from '../components/QtyEditor.svelte'
  import Icon from '../components/Icon.svelte'
  import { app } from '../lib/store.svelte'
  import { api } from '../lib/api'
  import type { Item, Tag } from '../lib/types'
  import { itemLow, itemMissing, tagMissing, qtyLabel, unitLabel, contentLabel } from '../lib/inv'
  import { fmt2 } from '../lib/util'
  import { burstFrom } from '../lib/fx'
  import { tada } from '../lib/sound'

  const lowItems = $derived(app.inventory.items.filter(itemLow).sort((a, b) => a.name.localeCompare(b.name)))
  const lowTags = $derived(app.inventory.tags.filter((t) => t.below_min).sort((a, b) => a.name.localeCompare(b.name)))
  const total = $derived(lowItems.length + lowTags.length)

  const FACT: Record<string, [string, number]> = { ml: ['v', 1], L: ['v', 1000], g: ['m', 1], kg: ['m', 1000] }
  /** quanto de "1 item" existe na unidade da tag */
  function perItemInTag(i: Item, t: Tag): number | null {
    if (t.unit === 'un') return 1
    if (!i.content_amount || !i.content_unit) return null
    const a = FACT[i.content_unit]
    const b = FACT[t.unit]
    if (!a || !b || a[0] !== b[0]) return null
    return (i.content_amount * a[1]) / b[1]
  }

  let buyOpen = $state(false)
  let buyTag = $state<Tag | null>(null)
  let buyItem = $state<Item | null>(null)
  let buyQty = $state(1)
  let saving = $state(false)
  let anchor: Element | null = null

  function suggest(i: Item, t: Tag | null) {
    if (t) {
      const per = perItemInTag(i, t)
      return per ? Math.max(1, Math.ceil(tagMissing(t) / per)) : 1
    }
    return Math.max(1, Math.ceil(itemMissing(i)))
  }

  function startItem(i: Item, e: Event) {
    anchor = e.currentTarget as Element
    buyTag = null
    buyItem = i
    buyQty = suggest(i, null)
    buyOpen = true
  }
  function startTag(t: Tag, e: Event) {
    anchor = e.currentTarget as Element
    buyTag = t
    const items = app.inventory.items.filter((i) => i.tag_ids.includes(t.id) && perItemInTag(i, t) != null)
    buyItem = items[0] ?? null
    buyQty = buyItem ? suggest(buyItem, t) : 1
    buyOpen = true
  }
  function pickItem(i: Item) {
    buyItem = i
    buyQty = suggest(i, buyTag)
  }

  async function confirmBuy() {
    if (!buyItem || buyQty <= 0) return
    saving = true
    try {
      await api(`/api/items/${buyItem.id}/adjust`, { body: { delta: buyQty, reason: 'compra' } })
      tada()
      burstFrom(anchor, 'confetti', 40)
      app.toast(`+${fmt2(buyQty)} ${unitLabel(buyItem.count_unit, buyQty)} de ${buyItem.name}`, 'ok', '🛒')
      buyOpen = false
      app.loadInventory()
    } catch (e) {
      app.fail(e)
    } finally {
      saving = false
    }
  }

  async function share() {
    const lines = ['🛒 Compras do Angel One', '']
    for (const t of lowTags) lines.push(`• ${t.emoji ?? ''} ${t.name}: faltam ${fmt2(tagMissing(t))} ${t.unit}`)
    for (const i of lowItems) lines.push(`• ${i.name}: ${fmt2(Math.ceil(itemMissing(i)))} ${unitLabel(i.count_unit, Math.ceil(itemMissing(i)))}`)
    const text = lines.join('\n')
    try {
      if (navigator.share) await navigator.share({ text })
      else {
        await navigator.clipboard.writeText(text)
        app.toast('Lista copiada!', 'ok', '📋')
      }
    } catch {
      /* cancelado */
    }
  }

  const tagItems = $derived(buyTag ? app.inventory.items.filter((i) => i.tag_ids.includes(buyTag!.id) && perItemInTag(i, buyTag!) != null) : [])
</script>

{#if !total}
  <div class="empty card">
    <div class="art">🧺</div>
    <p><b>Nada para comprar!</b></p>
    <p class="small">Tudo acima do mínimo. Itens e grupos com estoque mínimo aparecem aqui quando acabam.</p>
  </div>
{:else}
  <div class="head">
    <p class="muted small">{total} {total === 1 ? 'coisa' : 'coisas'} abaixo do mínimo</p>
    <button class="btn sm soft" onclick={share}><Icon name="share" size={16} /> Enviar lista</button>
  </div>

  <div class="list stagger">
    {#each lowTags as t (t.id)}
      <div class="row-c card">
        <span class="big-e">{t.emoji ?? '🏷️'}</span>
        <div class="grow">
          <div class="nm">{t.name} <span class="pill grp">grupo</span></div>
          <div class="small muted">tem {fmt2(t.total)} {t.unit} · mínimo {fmt2(t.min_qty ?? 0)} {t.unit}</div>
          <div class="miss">faltam <b>{fmt2(tagMissing(t))} {t.unit}</b></div>
        </div>
        {#if app.isCrew}<button class="btn sm gold" onclick={(e) => startTag(t, e)}>Comprei</button>{/if}
      </div>
    {/each}
    {#each lowItems as i (i.id)}
      <div class="row-c card">
        {#if i.photo}<img src={i.photo} alt="" />{:else}<span class="big-e">📦</span>{/if}
        <div class="grow">
          <div class="nm">{i.name}</div>
          <div class="small muted">tem {qtyLabel(i)} · mínimo {fmt2(i.min_qty ?? 0)}</div>
          <div class="miss">comprar <b>{fmt2(Math.ceil(itemMissing(i)))} {unitLabel(i.count_unit, Math.ceil(itemMissing(i)))}</b></div>
        </div>
        {#if app.isCrew}<button class="btn sm gold" onclick={(e) => startItem(i, e)}>Comprei</button>{/if}
      </div>
    {/each}
  </div>
{/if}

<Sheet bind:open={buyOpen} title="Quanto você comprou?">
  {#if buyTag}
    <div class="label" style="margin-bottom:8px">Qual {buyTag.name.toLowerCase()}?</div>
    {#if tagItems.length}
      <div class="pick">
        {#each tagItems as i (i.id)}
          <button class="pk" class:on={buyItem?.id === i.id} onclick={() => pickItem(i)}>
            {#if i.photo}<img src={i.photo} alt="" />{:else}<span>📦</span>{/if}
            <span class="pn">{i.name}</span>
            {#if contentLabel(i)}<small>{contentLabel(i)}</small>{/if}
          </button>
        {/each}
      </div>
    {:else}
      <p class="small muted">Não há itens com unidade compatível. Na despensa, cadastre a marca com o conteúdo de cada embalagem e associe a este grupo.</p>
    {/if}
  {/if}
  {#if buyItem}
    <p class="center" style="margin:14px 0 10px"><b>{buyItem.name}</b></p>
    <QtyEditor bind:value={buyQty} unit={buyItem.count_unit} />
    <p class="center small muted" style="margin-top:8px">Fica com {fmt2(buyItem.quantity + buyQty)} {unitLabel(buyItem.count_unit, buyItem.quantity + buyQty)}</p>
  {/if}
  {#snippet footer()}
    <button class="btn block" onclick={confirmBuy} disabled={!buyItem || buyQty <= 0 || saving}>Somar ao estoque</button>
  {/snippet}
</Sheet>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }
  .list {
    display: grid;
    gap: 10px;
  }
  .row-c {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
  }
  .row-c img {
    width: 54px;
    height: 54px;
    border-radius: 14px;
    object-fit: contain;
    background: #fff;
  }
  .big-e {
    width: 54px;
    height: 54px;
    display: grid;
    place-items: center;
    font-size: 30px;
    border-radius: 14px;
    background: var(--card-2);
    flex: none;
  }
  .nm {
    font-weight: 800;
  }
  .grp {
    color: var(--accent);
    font-size: 11px;
  }
  .miss {
    margin-top: 2px;
    color: var(--bad);
    font-size: 14px;
  }
  .pick {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .pk {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px;
    border-radius: 16px;
    border: 2px solid var(--line);
    background: var(--card);
    text-align: center;
  }
  .pk.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, var(--card));
  }
  .pk img {
    width: 56px;
    height: 56px;
    object-fit: contain;
    border-radius: 10px;
    background: #fff;
  }
  .pn {
    font-weight: 800;
    font-size: 12.5px;
    line-height: 1.15;
  }
  .pk small {
    color: var(--muted);
    font-size: 11px;
  }
</style>
