<script lang="ts">
  import { app } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import type { Item, Tag } from '../lib/types'
  import { qtyLabel, tagTotalLabel, itemLow, contentLabel, fillLevel } from '../lib/inv'
  import { fmt2 } from '../lib/util'
  import Icon from '../components/Icon.svelte'
  import Bottle from '../components/Bottle.svelte'
  import ItemSheet from './ItemSheet.svelte'
  import ItemForm from './ItemForm.svelte'
  import TagForm from './TagForm.svelte'
  import Compras from './Compras.svelte'
  import Conferir from './Conferir.svelte'

  const sub = $derived(router.path.split('/')[2] ?? '')
  let q = $state('')
  let tagFilter = $state<number | null>(null)

  const lowCount = $derived(
    app.inventory.items.filter(itemLow).length + app.inventory.tags.filter((t) => t.below_min).length,
  )
  const tags = $derived([...app.inventory.tags].sort((a, b) => a.name.localeCompare(b.name)))
  const filtered = $derived(
    app.inventory.items
      .filter((i) => (tagFilter == null || i.tag_ids.includes(tagFilter)) && (!q.trim() || i.name.toLowerCase().includes(q.trim().toLowerCase())))
      .sort((a, b) => Number(itemLow(b)) - Number(itemLow(a)) || a.name.localeCompare(b.name)),
  )
  const activeTag = $derived(tagFilter != null ? app.inventory.tags.find((t) => t.id === tagFilter) : null)

  let sel = $state<Item | null>(null)
  let sheetOpen = $state(false)
  let formOpen = $state(false)
  let editing = $state<Item | null>(null)
  let tagOpen = $state(false)
  let editingTag = $state<Tag | null>(null)

  function openItem(i: Item) {
    sel = i
    sheetOpen = true
  }
  function newItem() {
    editing = null
    formOpen = true
  }
  function editItem(i: Item) {
    sheetOpen = false
    editing = i
    setTimeout(() => (formOpen = true), 250)
  }
  function editTag(t: Tag | null) {
    editingTag = t
    tagOpen = true
  }
  // mantém o item aberto sincronizado com o servidor
  $effect(() => {
    if (sel) {
      const fresh = app.inventory.items.find((i) => i.id === sel!.id)
      if (fresh && fresh !== sel) sel = fresh
    }
  })
</script>

{#if sub === 'conferir' && app.isCrew}
  <Conferir />
{:else}
  <div class="page">
    <div class="page-title">
      <div class="crate"><Icon name="box" size={40} stroke={1.8} /></div>
      <div class="grow">
        <h1>Despensa</h1>
        <div class="sub">{app.inventory.items.length} {app.inventory.items.length === 1 ? 'item' : 'itens'} · {app.inventory.tags.length} {app.inventory.tags.length === 1 ? 'grupo' : 'grupos'}</div>
      </div>
      {#if app.isCrew && sub !== 'compras'}
        <button class="icon-btn add" onclick={newItem} aria-label="Novo item"><Icon name="plus" /></button>
      {/if}
    </div>

    <div class="tabs">
      <button class:on={sub === ''} onclick={() => router.go('/despensa', true)}>Estoque</button>
      <button class:on={sub === 'compras'} onclick={() => router.go('/despensa/compras', true)}>
        Compras {#if lowCount}<span class="b">{lowCount}</span>{/if}
      </button>
      {#if app.isCrew}
        <button onclick={() => router.go('/despensa/conferir')}>Conferir <Icon name="clipboard" size={16} /></button>
      {/if}
    </div>

    {#if sub === 'compras'}
      <Compras />
    {:else}
      <div class="search">
        <Icon name="search" size={18} />
        <input placeholder="Buscar na despensa" bind:value={q} type="search" />
      </div>

      {#if tags.length}
        <div class="tagrow">
          {#each tags as t (t.id)}
            <button class="tagcard" class:on={tagFilter === t.id} class:low={t.below_min} onclick={() => (tagFilter = tagFilter === t.id ? null : t.id)}>
              <span class="te">{t.emoji || '🏷️'}</span>
              <span class="tn">{t.name}</span>
              <span class="tt">{tagTotalLabel(t)}</span>
              {#if t.min_qty != null}
                <span class="tbar"><i style="width:{Math.min(100, (t.total / Math.max(t.min_qty, 0.0001)) * 50)}%"></i><em></em></span>
              {/if}
            </button>
          {/each}
          {#if app.isCrew}
            <button class="tagcard new" onclick={() => editTag(null)}><span class="te">＋</span><span class="tn">Novo grupo</span></button>
          {/if}
        </div>
      {:else if app.isCrew}
        <button class="btn soft sm" style="margin:6px 0 10px" onclick={() => editTag(null)}><Icon name="tag" size={16} /> Criar grupo (ex.: Vodka, Cerveja)</button>
      {/if}

      {#if activeTag}
        <div class="tag-head">
          <span>{activeTag.emoji} <b>{activeTag.name}</b>: {tagTotalLabel(activeTag)} no total{#if activeTag.min_qty != null} · mínimo {fmt2(activeTag.min_qty)} {activeTag.unit}{/if}</span>
          {#if app.isCrew}<button class="lnk" onclick={() => editTag(activeTag)}>editar</button>{/if}
        </div>
      {/if}

      <div class="grid stagger">
        {#each filtered as i (i.id)}
          <button class="it" class:low={itemLow(i)} onclick={() => openItem(i)}>
            <div class="ph">
              {#if i.photo}<img src={i.photo} alt="" loading="lazy" />{:else}<span class="noph"><Bottle level={fillLevel(i)} size={70} /></span>{/if}
              {#if itemLow(i)}<span class="lowb">Comprar</span>{/if}
            </div>
            <div class="nm">{i.name}</div>
            <div class="qt">
              <b>{qtyLabel(i)}</b>
              {#if contentLabel(i)}<span class="muted"> · {contentLabel(i)}</span>{/if}
            </div>
          </button>
        {/each}
      </div>
      {#if !filtered.length}
        <div class="empty card">
          <div class="art">📦</div>
          <p>{app.inventory.items.length ? 'Nada encontrado.' : 'A despensa está vazia.'}</p>
          {#if app.isCrew && !app.inventory.items.length}<button class="btn" style="margin-top:14px" onclick={newItem}>Cadastrar primeiro item</button>{/if}
        </div>
      {/if}
    {/if}
  </div>
{/if}

<ItemSheet bind:open={sheetOpen} item={sel} onedit={editItem} />
{#if app.isCrew}
  <ItemForm bind:open={formOpen} item={editing} presetTag={tagFilter} />
  <TagForm bind:open={tagOpen} tag={editingTag} />
{/if}

<style>
  .crate {
    color: var(--accent);
    animation: bob 4s ease-in-out infinite;
  }
  .add {
    background: var(--accent);
    color: var(--accent-ink);
    border: 0;
    box-shadow: 0 8px 18px -8px var(--accent);
  }
  .tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 14px;
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 10px 12px;
    font-weight: 800;
    color: var(--muted);
  }
  .tabs button.on {
    color: var(--ink);
  }
  .tabs button.on::after {
    content: '';
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 3px;
    border-radius: 3px;
    background: var(--accent);
    animation: growx 0.4s var(--spring);
  }
  @keyframes growx {
    from {
      transform: scaleX(0);
    }
  }
  .b {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 99px;
    background: var(--bad);
    color: #fff;
    font-size: 11.5px;
    display: inline-grid;
    place-items: center;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    height: 48px;
    border-radius: 16px;
    background: var(--card);
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .search input {
    flex: 1;
    border: 0;
    background: none;
    outline: none;
    height: 100%;
  }
  .tagrow {
    display: flex;
    gap: 10px;
    overflow-x: auto;
    scrollbar-width: none;
    padding: 12px 18px 6px;
    margin: 0 -18px;
  }
  .tagrow::-webkit-scrollbar {
    display: none;
  }
  .tagcard {
    flex: none;
    min-width: 108px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 11px 13px;
    border-radius: 18px;
    background: var(--card);
    border: 1.5px solid var(--line);
    text-align: left;
    transition: transform 0.3s var(--spring), border-color 0.2s, background 0.2s;
  }
  .tagcard:active {
    transform: scale(0.95);
  }
  .tagcard.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, var(--card));
  }
  .tagcard.low .tt {
    color: var(--bad);
  }
  .tagcard.new {
    justify-content: center;
    color: var(--accent);
    border-style: dashed;
  }
  .te {
    font-size: 22px;
  }
  .tn {
    font-weight: 800;
    font-size: 14px;
  }
  .tt {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 17px;
  }
  .tbar {
    position: relative;
    width: 100%;
    height: 5px;
    border-radius: 5px;
    background: var(--line);
    margin-top: 5px;
    overflow: hidden;
  }
  .tbar i {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: 5px;
  }
  .tagcard.low .tbar i {
    background: var(--bad);
  }
  .tbar em {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--ink);
    opacity: 0.4;
  }
  .tag-head {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    font-size: 14px;
    padding: 6px 4px 2px;
  }
  .lnk {
    color: var(--accent);
    font-weight: 800;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    margin-top: 12px;
  }
  .it {
    text-align: left;
    transition: transform 0.3s var(--spring);
  }
  .it:active {
    transform: scale(0.94);
  }
  .ph {
    position: relative;
    aspect-ratio: 1;
    border-radius: 20px;
    background: #fff;
    box-shadow: var(--shadow);
    overflow: hidden;
    display: grid;
    place-items: center;
  }
  :global(html[data-band='noite']) .ph {
    background: #f4f6fb;
  }
  .ph img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .noph {
    display: grid;
    place-items: center;
  }
  .it.low .ph {
    box-shadow: 0 0 0 2.5px var(--bad), var(--shadow);
  }
  .lowb {
    position: absolute;
    top: 6px;
    right: 6px;
    padding: 2px 7px;
    border-radius: 99px;
    background: var(--bad);
    color: #fff;
    font-size: 10.5px;
    font-weight: 900;
  }
  .nm {
    margin-top: 7px;
    font-weight: 800;
    font-size: 13.5px;
    line-height: 1.2;
  }
  .qt {
    font-size: 12.5px;
    margin-top: 1px;
  }
</style>
