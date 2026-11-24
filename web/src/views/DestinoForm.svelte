<script lang="ts">
  import Sheet from '../components/Sheet.svelte'
  import PhotoInput from '../components/PhotoInput.svelte'
  import LocationPicker from '../components/LocationPicker.svelte'
  import WindRose from '../components/WindRose.svelte'
  import { app } from '../lib/store.svelte'
  import { api } from '../lib/api'
  import { router } from '../lib/router.svelte'
  import type { Destination, Dir } from '../lib/types'
  import { DIR_NOME } from '../lib/util'
  import { burst } from '../lib/fx'
  import { tada } from '../lib/sound'

  let { open = $bindable(false), dest = null as Destination | null }: { open?: boolean; dest?: Destination | null } = $props()

  let name = $state('')
  let photo = $state<string | null>(null)
  let original = $state<string | null>(null)
  let lat = $state<number | null>(null)
  let lon = $state<number | null>(null)
  let winds = $state<Dir[]>([])
  let busy = $state(false)
  let photoBusy = $state(false)
  let key = $state(0)

  $effect(() => {
    if (open) {
      name = dest?.name ?? ''
      photo = dest?.photo ?? null
      original = dest?.photo_original ?? null
      lat = dest?.lat ?? null
      lon = dest?.lon ?? null
      winds = [...(dest?.good_winds ?? [])]
      key = dest?.id ?? 0
    }
  })

  async function save() {
    if (!name.trim() || lat == null || lon == null) return
    busy = true
    try {
      const body = { name: name.trim(), lat, lon, photo, photo_original: original, good_winds: winds }
      if (dest) await api(`/api/destinations/${dest.id}`, { method: 'PATCH', body })
      else {
        await api('/api/destinations', { body })
        tada()
        burst('confetti', innerWidth / 2, innerHeight * 0.4)
      }
      app.toast(dest ? 'Destino atualizado' : 'Destino criado!', 'ok', '🏝️')
      open = false
      app.loadDestinations()
    } catch (e) {
      app.fail(e)
    } finally {
      busy = false
    }
  }

  async function remove() {
    if (!dest || !confirm(`Apagar "${dest.name}"? Os votos dele também somem.`)) return
    try {
      await api(`/api/destinations/${dest.id}`, { method: 'DELETE' })
      app.toast('Destino apagado', 'info', '🗑️')
      open = false
      router.go('/destinos', true)
      app.loadDestinations()
    } catch (e) {
      app.fail(e)
    }
  }
</script>

<Sheet bind:open title={dest ? 'Editar destino' : 'Novo destino'} full>
  {#key key}
    <label class="field">
      <span>Nome</span>
      <input class="input" bind:value={name} placeholder="Ex.: Praia do Forte" maxlength="60" />
    </label>

    <div class="field">
      <span>Foto</span>
      <PhotoInput bind:photo bind:original bind:busy={photoBusy} mode="enhance" aspect="16 / 10" />
    </div>

    <div class="field">
      <span>Onde fica (toque no ponto ou arraste o mapa)</span>
      <LocationPicker bind:lat bind:lon />
      <small class="muted">Dica: marque o ponto de fundeio na água, um pouco afastado da areia.</small>
    </div>

    <div class="field">
      <span>Bom para ventos de…</span>
      <p class="small muted">Toque nas direções de onde vem o vento contra o qual este lugar fica abrigado.</p>
      <WindRose bind:value={winds} editable size={250} />
      <p class="small center" style="margin-top:6px">
        {winds.length ? winds.map((w) => DIR_NOME[w]).join(' · ') : 'Nenhuma direção marcada'}
      </p>
    </div>
  {/key}

  {#snippet footer()}
    <div class="row">
      {#if dest}<button class="btn danger" onclick={remove} aria-label="Apagar">Apagar</button>{/if}
      <button class="btn grow" onclick={save} disabled={busy || photoBusy || !name.trim() || lat == null}>
        {busy ? 'Salvando…' : photoBusy ? 'Aguardando a foto…' : 'Salvar destino'}
      </button>
    </div>
  {/snippet}
</Sheet>
