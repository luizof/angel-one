<script lang="ts">
  import Sheet from '../components/Sheet.svelte'
  import { app } from '../lib/store.svelte'
  import { api } from '../lib/api'
  import type { Tag, TagUnit } from '../lib/types'

  let { open = $bindable(false), tag = null as Tag | null }: { open?: boolean; tag?: Tag | null } = $props()

  const EMOJIS = ['🍸', '🍺', '🍷', '🥃', '🍾', '🧉', '🥤', '💧', '🧊', '🍋', '🍓', '🧂', '🍬', '🥖', '🧀', '🥩', '🐟', '🍤', '🍟', '🥜', '☕', '🧻', '🧴', '🧽', '⛽', '🏷️']
  let name = $state('')
  let emoji = $state('🏷️')
  let unit = $state<TagUnit>('L')
  let minQty = $state<number | null>(null)
  let busy = $state(false)

  $effect(() => {
    if (open) {
      name = tag?.name ?? ''
      emoji = tag?.emoji ?? '🏷️'
      unit = tag?.unit ?? 'L'
      minQty = tag?.min_qty ?? null
    }
  })

  async function save() {
    if (!name.trim()) return
    busy = true
    try {
      const body = { name: name.trim(), emoji, unit, min_qty: minQty === null || (minQty as unknown) === '' ? null : Number(minQty) }
      if (tag) await api(`/api/tags/${tag.id}`, { method: 'PATCH', body })
      else await api('/api/tags', { body })
      app.toast(tag ? 'Grupo atualizado' : 'Grupo criado', 'ok', emoji)
      open = false
      app.loadInventory()
    } catch (e) {
      app.fail(e)
    } finally {
      busy = false
    }
  }
  async function remove() {
    if (!tag || !confirm(`Apagar o grupo "${tag.name}"? Os itens continuam na despensa.`)) return
    try {
      await api(`/api/tags/${tag.id}`, { method: 'DELETE' })
      open = false
      app.loadInventory()
    } catch (e) {
      app.fail(e)
    }
  }
</script>

<Sheet bind:open title={tag ? 'Editar grupo' : 'Novo grupo'}>
  <label class="field">
    <span>Nome</span>
    <input class="input" bind:value={name} placeholder="Ex.: Vodka, Cerveja, Açúcar" maxlength="30" />
  </label>
  <div class="label" style="margin-bottom:8px">Ícone</div>
  <div class="emojis">
    {#each EMOJIS as e}<button class="emo" class:on={emoji === e} onclick={() => (emoji = e)}>{e}</button>{/each}
  </div>
  <div class="two">
    <label class="field">
      <span>Somar em</span>
      <select class="input" bind:value={unit}>
        <option value="L">litros (L)</option>
        <option value="ml">mililitros (ml)</option>
        <option value="kg">quilos (kg)</option>
        <option value="g">gramas (g)</option>
        <option value="un">unidades</option>
      </select>
    </label>
    <label class="field">
      <span>Mínimo do grupo</span>
      <input class="input" type="number" inputmode="decimal" step="any" min="0" bind:value={minQty} placeholder="opcional" />
    </label>
  </div>
  <p class="small muted">Ex.: Vodka em litros com mínimo 2 → entra na lista de compras quando a soma de todas as marcas ficar abaixo de 2 L.</p>
  {#snippet footer()}
    <div class="row">
      {#if tag}<button class="btn danger" onclick={remove}>Apagar</button>{/if}
      <button class="btn grow" onclick={save} disabled={busy || !name.trim()}>Salvar grupo</button>
    </div>
  {/snippet}
</Sheet>

<style>
  .emojis {
    display: grid;
    grid-template-columns: repeat(9, 1fr);
    gap: 5px;
    margin-bottom: 16px;
  }
  .emo {
    aspect-ratio: 1;
    border-radius: 12px;
    font-size: 20px;
    background: var(--card);
    border: 1.5px solid var(--line);
  }
  .emo.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .two {
    display: grid;
    grid-template-columns: 1.2fr 1fr;
    gap: 10px;
  }
</style>
