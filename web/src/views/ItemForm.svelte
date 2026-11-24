<script lang="ts">
  import Sheet from '../components/Sheet.svelte'
  import PhotoInput from '../components/PhotoInput.svelte'
  import QtyEditor from '../components/QtyEditor.svelte'
  import { app } from '../lib/store.svelte'
  import { api } from '../lib/api'
  import type { ContentUnit, Item, Tag, TagUnit } from '../lib/types'
  import { COUNT_UNITS } from '../lib/inv'
  import { burst } from '../lib/fx'
  import { tada } from '../lib/sound'

  let { open = $bindable(false), item = null as Item | null, presetTag = null as number | null }: { open?: boolean; item?: Item | null; presetTag?: number | null } = $props()

  let name = $state('')
  let photo = $state<string | null>(null)
  let original = $state<string | null>(null)
  let tagIds = $state<number[]>([])
  let countUnit = $state('garrafa')
  let contentAmount = $state<number | null>(null)
  let contentUnit = $state<ContentUnit | ''>('ml')
  let fractional = $state(true)
  let quantity = $state(1)
  let minQty = $state<number | null>(null)
  let busy = $state(false)
  let photoBusy = $state(false)
  let key = $state(0)
  let newTag = $state('')
  let newTagUnit = $state<TagUnit>('L')
  let addingTag = $state(false)

  $effect(() => {
    if (open) {
      name = item?.name ?? ''
      photo = item?.photo ?? null
      original = item?.photo_original ?? null
      tagIds = [...(item?.tag_ids ?? (presetTag != null ? [presetTag] : []))]
      countUnit = item?.count_unit ?? 'garrafa'
      contentAmount = item?.content_amount ?? null
      contentUnit = item?.content_unit ?? 'ml'
      fractional = item?.fractional ?? true
      quantity = item?.quantity ?? 1
      minQty = item?.min_qty ?? null
      addingTag = false
      key = item?.id ?? 0
    }
  })

  // sugestão: garrafas e galões costumam ser fracionados
  function setUnit(u: string) {
    countUnit = u
    if (!item) fractional = ['garrafa', 'galão', 'vidro', 'pote'].includes(u)
  }

  function toggleTag(id: number) {
    tagIds = tagIds.includes(id) ? tagIds.filter((x) => x !== id) : [...tagIds, id]
  }

  async function createTag() {
    if (!newTag.trim()) return
    try {
      const t = await api<Tag>('/api/tags', { body: { name: newTag.trim(), unit: newTagUnit, emoji: null, min_qty: null } })
      tagIds = [...tagIds, t.id]
      newTag = ''
      addingTag = false
      app.loadInventory()
    } catch (e) {
      app.fail(e)
    }
  }

  async function save() {
    if (!name.trim()) return
    busy = true
    try {
      const body = {
        name: name.trim(), photo, photo_original: original, tag_ids: tagIds, count_unit: countUnit,
        content_amount: contentAmount || null, content_unit: contentAmount ? contentUnit || null : null,
        fractional, min_qty: minQty === null || (minQty as unknown) === '' ? null : Number(minQty),
      }
      if (item) {
        await api(`/api/items/${item.id}`, { method: 'PATCH', body })
        if (Math.abs(quantity - item.quantity) > 1e-9) await api(`/api/items/${item.id}/adjust`, { body: { quantity, reason: 'ajuste' } })
      } else {
        await api('/api/items', { body: { ...body, quantity } })
        tada()
        burst('confetti', innerWidth / 2, innerHeight * 0.4, 60)
      }
      app.toast(item ? 'Item atualizado' : 'Item na despensa!', 'ok', '📦')
      open = false
      app.loadInventory()
    } catch (e) {
      app.fail(e)
    } finally {
      busy = false
    }
  }

  async function remove() {
    if (!item || !confirm(`Apagar "${item.name}" da despensa?`)) return
    try {
      await api(`/api/items/${item.id}`, { method: 'DELETE' })
      app.toast('Item apagado', 'info', '🗑️')
      open = false
      app.loadInventory()
    } catch (e) {
      app.fail(e)
    }
  }

  const tags = $derived([...app.inventory.tags].sort((a, b) => a.name.localeCompare(b.name)))
</script>

<Sheet bind:open title={item ? 'Editar item' : 'Novo item'} full>
  {#key key}
    <div class="field">
      <span>Foto</span>
      <PhotoInput bind:photo bind:original bind:busy={photoBusy} mode="clean" aspect="1 / 1" />
    </div>
  {/key}

  <label class="field">
    <span>Nome</span>
    <input class="input" bind:value={name} placeholder="Ex.: Vodka Absolut 1 L" maxlength="60" />
  </label>

  <div class="field">
    <span>Grupos (tags)</span>
    <div class="tags">
      {#each tags as t (t.id)}
        <button class="chip" class:on={tagIds.includes(t.id)} onclick={() => toggleTag(t.id)}>{t.emoji ?? '🏷️'} {t.name} <small>({t.unit})</small></button>
      {/each}
      <button class="chip add" onclick={() => (addingTag = !addingTag)}>＋ novo grupo</button>
    </div>
    {#if addingTag}
      <div class="newtag">
        <input class="input" bind:value={newTag} placeholder="Ex.: Vodka" maxlength="30" />
        <select class="input" bind:value={newTagUnit}>
          <option value="L">litros</option>
          <option value="ml">ml</option>
          <option value="kg">kg</option>
          <option value="g">g</option>
          <option value="un">unidades</option>
        </select>
        <button class="btn sm" onclick={createTag} disabled={!newTag.trim()}>Criar</button>
      </div>
      <small class="muted">A unidade do grupo é usada para somar tudo (ex.: Vodka em litros soma garrafas de 750 ml e de 1 L).</small>
    {/if}
  </div>

  <div class="two">
    <label class="field">
      <span>Conta-se em</span>
      <select class="input" value={countUnit} onchange={(e) => setUnit((e.target as HTMLSelectElement).value)}>
        {#each COUNT_UNITS as u}<option value={u}>{u}</option>{/each}
      </select>
    </label>
    <div class="field">
      <span>Conteúdo de cada</span>
      <div class="content">
        <input class="input" type="number" inputmode="decimal" step="any" min="0" bind:value={contentAmount} placeholder="750" />
        <select class="input" bind:value={contentUnit}>
          <option value="ml">ml</option>
          <option value="L">L</option>
          <option value="g">g</option>
          <option value="kg">kg</option>
          <option value="un">un</option>
        </select>
      </div>
    </div>
  </div>

  <button class="frac" onclick={() => (fractional = !fractional)}>
    <span class="switch" class:on={fractional}></span>
    <span><b>Pode ficar pela metade</b><small>Mostra o nível da {countUnit} aberta (¼, ½, ¾)</small></span>
  </button>

  <div class="field" style="margin-top:16px">
    <span>Quantidade agora</span>
    <QtyEditor bind:value={quantity} {fractional} unit={countUnit} />
  </div>

  <label class="field">
    <span>Estoque mínimo deste item (opcional)</span>
    <input class="input" type="number" inputmode="decimal" step="any" min="0" bind:value={minQty} placeholder="Deixe vazio se o mínimo for pelo grupo" />
  </label>

  {#snippet footer()}
    <div class="row">
      {#if item}<button class="btn danger" onclick={remove}>Apagar</button>{/if}
      <button class="btn grow" onclick={save} disabled={busy || photoBusy || !name.trim()}>{busy ? 'Salvando…' : photoBusy ? 'Aguardando a foto…' : 'Salvar item'}</button>
    </div>
  {/snippet}
</Sheet>

<style>
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .tags small {
    opacity: 0.6;
  }
  .chip.add {
    color: var(--accent);
    border-style: dashed;
  }
  .newtag {
    display: grid;
    grid-template-columns: 1fr 110px auto;
    gap: 8px;
    margin-top: 10px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1.3fr;
    gap: 10px;
  }
  .content {
    display: grid;
    grid-template-columns: 1fr 76px;
    gap: 6px;
  }
  .content .input {
    padding-right: 10px;
  }
  .frac {
    display: flex;
    align-items: center;
    gap: 12px;
    text-align: left;
  }
  .frac b {
    display: block;
  }
  .frac small {
    color: var(--muted);
    font-weight: 700;
  }
</style>
