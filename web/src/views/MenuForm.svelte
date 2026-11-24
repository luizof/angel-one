<script lang="ts">
  // Cadastro de item do cardápio: o LLM preenche descrição/receita e a IA cria a foto.
  import Sheet from '../components/Sheet.svelte'
  import PhotoInput from '../components/PhotoInput.svelte'
  import Icon from '../components/Icon.svelte'
  import { app } from '../lib/store.svelte'
  import { api, runJob } from '../lib/api'
  import type { Category, MenuItem } from '../lib/types'
  import { burst } from '../lib/fx'
  import { sparkle, tada } from '../lib/sound'

  let { open = $bindable(false), item = null as MenuItem | null, defaultCategory = 'drink' as Category }: { open?: boolean; item?: MenuItem | null; defaultCategory?: Category } = $props()

  let name = $state('')
  let category = $state<Category>('drink')
  let description = $state('')
  let recipe = $state('')
  let photo = $state<string | null>(null)
  let original = $state<string | null>(null)
  let available = $state(true)
  let busy = $state(false)
  let photoBusy = $state(false)
  let aiText = $state(false)
  let aiPhoto = $state(false)
  let key = $state(0)

  $effect(() => {
    if (open) {
      name = item?.name ?? ''
      category = item?.category ?? defaultCategory
      description = item?.description ?? ''
      recipe = item?.recipe ?? ''
      photo = item?.photo ?? null
      original = item?.photo ?? null
      available = item?.available ?? true
      key = item?.id ?? 0
    }
  })

  const canAi = $derived(!!app.settings?.openrouter_key_set)

  async function magic() {
    if (!name.trim()) return
    sparkle()
    const tasks: Promise<unknown>[] = []
    aiText = true
    tasks.push(
      runJob<{ description: string; recipe: string }>('/api/ai/menu-text', { name: name.trim(), category })
        .then((r) => {
          description = r.description ?? description
          recipe = r.recipe ?? recipe
        })
        .catch((e) => app.fail(e))
        .finally(() => (aiText = false)),
    )
    aiPhoto = true
    tasks.push(
      runJob<{ url: string }>('/api/ai/menu-photo', { name: name.trim(), category, description: description || name.trim() })
        .then((r) => {
          photo = r.url
          original = r.url
          key++
        })
        .catch((e) => app.fail(e))
        .finally(() => (aiPhoto = false)),
    )
    await Promise.allSettled(tasks)
    burst('sparkles', innerWidth / 2, innerHeight * 0.3, 24)
  }

  async function regenPhoto() {
    aiPhoto = true
    try {
      const r = await runJob<{ url: string }>('/api/ai/menu-photo', { name: name.trim(), category, description })
      photo = r.url
      original = r.url
      key++
    } catch (e) {
      app.fail(e)
    } finally {
      aiPhoto = false
    }
  }

  async function save() {
    if (!name.trim()) return
    busy = true
    try {
      const body = { name: name.trim(), category, description: description.trim(), recipe: recipe.trim(), photo, available }
      if (item) await api(`/api/menu/${item.id}`, { method: 'PATCH', body })
      else {
        await api('/api/menu', { body: { ...body, sort: 0 } })
        tada()
        burst('confetti', innerWidth / 2, innerHeight * 0.4)
      }
      app.toast(item ? 'Item atualizado' : 'Item no cardápio!', 'ok', '🍹')
      open = false
      app.loadMenu()
    } catch (e) {
      app.fail(e)
    } finally {
      busy = false
    }
  }

  async function remove() {
    if (!item || !confirm(`Tirar "${item.name}" do cardápio?`)) return
    try {
      await api(`/api/menu/${item.id}`, { method: 'DELETE' })
      app.toast('Item removido', 'info', '🗑️')
      open = false
      app.loadMenu()
    } catch (e) {
      app.fail(e)
    }
  }
</script>

<Sheet bind:open title={item ? 'Editar item' : 'Novo no cardápio'} full>
  <div class="cats">
    {#each [['drink', '🍹 Drink'], ['petisco', '🍤 Petisco'], ['bebida', '🥤 Bebida']] as [id, lbl]}
      <button class="chip" class:on={category === id} onclick={() => (category = id as Category)}>{lbl}</button>
    {/each}
  </div>

  <label class="field">
    <span>Nome</span>
    <input class="input big" bind:value={name} placeholder={category === 'drink' ? 'Ex.: Caipirinha de maracujá' : category === 'petisco' ? 'Ex.: Isca de peixe' : 'Ex.: Cerveja Corona'} maxlength="60" />
  </label>

  <button class="magic" onclick={magic} disabled={!name.trim() || aiText || aiPhoto || !canAi}>
    <span class="wand"><Icon name="sparkle" size={22} /></span>
    <span class="grow">
      <b>{aiText || aiPhoto ? 'O anjo está criando…' : 'Preencher com IA'}</b>
      <small>{canAi ? 'Descrição, receita e ilustração 2D automáticas — depois é só ajustar.' : 'Configure a chave do OpenRouter para usar a IA.'}</small>
    </span>
  </button>

  <div class="field">
    <span>Foto</span>
    {#if aiPhoto}
      <div class="skeleton ph-load"><span>📸 Fotografando o {category === 'petisco' ? 'petisco' : 'drink'}…</span></div>
    {:else}
      {#key key}
        <PhotoInput bind:photo bind:original bind:busy={photoBusy} mode="none" aspect="1 / 1" />
      {/key}
      {#if name.trim() && canAi}
        <button class="btn sm soft" style="margin-top:8px" onclick={regenPhoto}><Icon name="refresh" size={16} /> Gerar outra foto com IA</button>
      {/if}
    {/if}
  </div>

  <label class="field">
    <span>Descrição</span>
    {#if aiText}<div class="skeleton" style="height:50px"></div>{:else}<input class="input" bind:value={description} placeholder="Uma frase que dá vontade de pedir" maxlength="140" />{/if}
  </label>

  <label class="field">
    <span>Receita (só a tripulação vê)</span>
    {#if aiText}<div class="skeleton" style="height:160px"></div>{:else}<textarea class="input" bind:value={recipe} rows="8" placeholder={'Ingredientes:\n- …\n\nPreparo:\n1. …'}></textarea>{/if}
  </label>

  <button class="avail" onclick={() => (available = !available)}>
    <span class="switch" class:on={available}></span>
    <span>{available ? 'Disponível para pedir' : 'Indisponível'}</span>
  </button>

  {#snippet footer()}
    <div class="row">
      {#if item}<button class="btn danger" onclick={remove}>Remover</button>{/if}
      <button class="btn grow" onclick={save} disabled={busy || photoBusy || aiText || aiPhoto || !name.trim()}>{busy ? 'Salvando…' : 'Salvar'}</button>
    </div>
  {/snippet}
</Sheet>

<style>
  .cats {
    display: flex;
    gap: 8px;
    margin-bottom: 14px;
  }
  .input.big {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 600;
  }
  .magic {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 14px 16px;
    margin: 0 0 18px;
    border-radius: 20px;
    text-align: left;
    color: #fff;
    background: linear-gradient(120deg, #7b5cff, #ff6bb0, #ffb347, #7b5cff);
    background-size: 300% 100%;
    animation: shimmer 6s linear infinite;
    box-shadow: 0 12px 26px -12px #7b5cff;
    transition: transform 0.3s var(--spring), opacity 0.2s;
  }
  .magic:active {
    transform: scale(0.97);
  }
  .magic[disabled] {
    opacity: 0.55;
  }
  .magic b {
    display: block;
    font-family: var(--font-display);
    font-weight: 600;
    font-size: 18px;
  }
  .magic small {
    opacity: 0.9;
    font-weight: 700;
  }
  .wand {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.22);
    animation: wiggle 2.4s ease-in-out infinite;
  }
  .ph-load {
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    font-weight: 800;
    color: var(--muted);
  }
  .avail {
    display: flex;
    align-items: center;
    gap: 12px;
    font-weight: 800;
  }
</style>
