<script lang="ts">
  // Foto com IA: tirar/escolher → envia → (opcional) IA melhora ou coloca fundo branco.
  import { upload, runJob } from '../lib/api'
  import { app } from '../lib/store.svelte'
  import Icon from './Icon.svelte'
  import CropImage from './CropImage.svelte'

  let {
    photo = $bindable(null as string | null),
    original = $bindable(null as string | null),
    mode = 'none' as 'clean' | 'enhance' | 'none',
    aspect = '4 / 3',
    busy = $bindable(false),
  }: {
    photo?: string | null
    original?: string | null
    mode?: 'clean' | 'enhance' | 'none'
    aspect?: string
    busy?: boolean
  } = $props()

  let input: HTMLInputElement
  let preview = $state<string | null>(null)
  let stage = $state<'idle' | 'upload' | 'ai'>('idle')
  let showOriginal = $state(false)
  let cropFile = $state<File | null>(null)

  const aiLabel = $derived(mode === 'clean' ? 'Ilustrar com fundo branco' : 'Ilustrar o destino em 2D')
  const shown = $derived(showOriginal && original ? original : (preview ?? photo))
  const hasAi = $derived(!!photo && !!original && photo !== original)

  async function pick(e: Event) {
    const f = (e.target as HTMLInputElement).files?.[0]
    if (!f) return
    input.value = ''
    cropFile = f
  }

  function cancelCrop() { cropFile = null }
  async function acceptCrop(blob: Blob) {
    cropFile = null
    preview = URL.createObjectURL(blob)
    showOriginal = false
    busy = true
    stage = 'upload'
    try {
      const r = await upload(new File([blob], 'foto.jpg', { type: 'image/jpeg' }))
      original = r.url
      photo = r.url
      if (preview) URL.revokeObjectURL(preview)
      preview = null
      if (mode !== 'none' && app.settings?.openrouter_key_set) await runAi()
    } catch (err) {
      app.fail(err)
      if (preview) URL.revokeObjectURL(preview)
      preview = null
    } finally {
      stage = 'idle'
      busy = false
    }
  }

  async function runAi() {
    if (!original) return
    busy = true
    stage = 'ai'
    try {
      const r = await runJob<{ url: string }>(mode === 'clean' ? '/api/ai/clean-photo' : '/api/ai/enhance-photo', { url: original })
      photo = r.url
      showOriginal = false
    } catch (err) {
      app.fail(err)
    } finally {
      stage = 'idle'
      busy = false
    }
  }

  function useOriginal() {
    if (original) photo = original
    showOriginal = false
  }
</script>

<div class="photo-in">
  <button class="frame" class:empty={!shown} style="aspect-ratio:{aspect}" onclick={() => input.click()} disabled={stage !== 'idle'}>
    {#if shown}
      <img src={shown} alt="" class:clean={mode === 'clean'} />
    {:else}
      <span class="ph">
        <span class="cam"><Icon name="camera" size={34} /></span>
        <b>Tirar ou escolher foto</b>
        {#if mode !== 'none'}<small>{aiLabel} automaticamente ✨</small>{/if}
      </span>
    {/if}
    {#if stage !== 'idle'}
      <span class="work">
        <span class="sparkles">✦ ✧ ✦</span>
        <b>{stage === 'upload' ? 'Enviando foto…' : mode === 'clean' ? 'A IA está desenhando o item…' : 'A IA está ilustrando o lugar…'}</b>
      </span>
    {/if}
  </button>
  <input bind:this={input} type="file" accept="image/*" onchange={pick} hidden />
  {#if cropFile}<CropImage file={cropFile} {aspect} ondone={acceptCrop} oncancel={cancelCrop} />{/if}

  {#if photo && stage === 'idle'}
    <div class="acts">
      {#if hasAi}
        <div class="seg">
          <button class:on={showOriginal} onclick={() => (showOriginal = true)}>Original</button>
          <button class:on={!showOriginal} onclick={() => (showOriginal = false)}>IA ✨</button>
        </div>
        <button class="btn sm ghost" onclick={useOriginal}>Usar original</button>
      {:else if mode !== 'none' && original}
        <button class="btn sm soft" onclick={runAi} disabled={!app.settings?.openrouter_key_set}>
          <Icon name="sparkle" size={18} /> {aiLabel}
        </button>
      {/if}
      <button class="btn sm ghost" onclick={() => input.click()}><Icon name="camera" size={18} /> Trocar</button>
    </div>
    {#if mode !== 'none' && !app.settings?.openrouter_key_set}
      <p class="small muted" style="margin-top:6px">A IA fica disponível quando a chave do OpenRouter estiver configurada.</p>
    {/if}
  {/if}
</div>

<style>
  .frame {
    position: relative;
    display: block;
    width: 100%;
    border-radius: 22px;
    overflow: hidden;
    background: var(--card);
    border: 1.5px solid var(--line);
  }
  .frame.empty {
    border: 2px dashed color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 6%, transparent);
  }
  .frame img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: pop-in 0.5s var(--spring);
  }
  .frame img.clean {
    object-fit: contain;
    background: #fff;
  }
  .ph {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--accent);
  }
  .ph b {
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 600;
  }
  .ph small {
    color: var(--muted);
    font-weight: 700;
  }
  .cam {
    width: 66px;
    height: 66px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    animation: bob 3s ease-in-out infinite;
  }
  .work {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: #fff;
    background: linear-gradient(120deg, rgba(10, 30, 60, 0.55), rgba(10, 30, 60, 0.25), rgba(10, 30, 60, 0.55));
    background-size: 300% 100%;
    animation: shimmer 2s linear infinite;
    -webkit-backdrop-filter: blur(3px);
    backdrop-filter: blur(3px);
    text-shadow: 0 1px 6px rgba(0, 0, 0, 0.4);
  }
  .sparkles {
    font-size: 26px;
    color: var(--gold);
    letter-spacing: 8px;
    animation: bob 1.4s ease-in-out infinite;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
    align-items: center;
  }
  .seg {
    display: inline-flex;
    padding: 3px;
    border-radius: 99px;
    background: var(--line);
  }
  .seg button {
    padding: 7px 13px;
    border-radius: 99px;
    font-weight: 800;
    font-size: 14px;
    color: var(--muted);
    transition: background 0.2s, color 0.2s;
  }
  .seg button.on {
    background: var(--card-solid);
    color: var(--ink);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
  }
</style>
