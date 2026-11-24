<script lang="ts">
  import { onMount } from 'svelte'
  import { app } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { api } from '../lib/api'
  import type { Band, Crew, Settings } from '../lib/types'
  import { BAND_INFO, isStandalone } from '../lib/util'
  import { soundEnabled, setSoundEnabled, chime } from '../lib/sound'
  import ModelPicker from '../components/ModelPicker.svelte'
  import Icon from '../components/Icon.svelte'

  let s = $state<Partial<Settings>>({})
  let saving = $state(false)
  let modelsErr = $state('')
  let sound = $state(soundEnabled())

  onMount(async () => {
    await app.loadSettings()
    if (app.settings) s = { ...app.settings }
    app.loadCrew()
    try {
      await app.loadModels()
    } catch (e) {
      modelsErr = e instanceof Error ? e.message : 'Não consegui carregar a lista de modelos.'
    }
  })

  const imageEditModels = $derived((app.models?.image ?? []).filter((m) => m.image_input !== false))

  async function save() {
    saving = true
    try {
      const body: Record<string, unknown> = {
        text_model: s.text_model, image_model: s.image_model, edit_model: s.edit_model,
        cruise_speed_kn: Number(s.cruise_speed_kn), moving_threshold_kn: Number(s.moving_threshold_kn),
      }
      await api('/api/settings', { method: 'PUT', body })
      await app.loadSettings()
      app.toast('Configurações salvas', 'ok', '⚙️')
    } catch (e) {
      app.fail(e)
    } finally {
      saving = false
    }
  }

  async function rename(c: Crew) {
    const n = prompt('Novo nome', c.name)
    if (!n?.trim()) return
    try {
      await api(`/api/crew/${c.id}`, { method: 'PATCH', body: { name: n.trim() } })
      if (app.crew?.id === c.id) app.becomeCrew({ ...app.crew, name: n.trim() })
      app.loadCrew()
    } catch (e) {
      app.fail(e)
    }
  }
  async function removeCrew(c: Crew) {
    if (!confirm(`Remover ${c.name} da tripulação?`)) return
    try {
      await api(`/api/crew/${c.id}`, { method: 'DELETE' })
      app.loadCrew()
      if (app.crew?.id === c.id) leave()
    } catch (e) {
      app.fail(e)
    }
  }
  function leave() {
    app.leaveCrew()
    app.toast('Você voltou a ser passageiro', 'info', '🏖️')
    router.go('/')
  }

  const BANDS: Band[] = ['manha', 'tarde', 'final_de_tarde', 'noite']
</script>

<div class="page">
  <div class="page-title">
    <button class="icon-btn" onclick={() => router.go('/')} aria-label="Voltar"><Icon name="back" /></button>
    <div class="grow">
      <h1>Ajustes</h1>
      <div class="sub">Só a tripulação vê esta tela</div>
    </div>
  </div>

  <div class="me card pad">
    <span class="em">{app.crew?.emoji}</span>
    <div class="grow">
      <b>{app.crew?.name}</b>
      <div class="small muted">Modo tripulante neste aparelho</div>
    </div>
    <button class="btn sm danger" onclick={leave}><Icon name="logout" size={16} /> Sair</button>
  </div>

  <h2 class="sh">✨ Inteligência artificial</h2>
  <div class="card pad">
    <div class="keyrow">
      <span class="dot" class:ok={app.settings?.openrouter_key_set}></span>
      <span class="grow">{app.settings?.openrouter_key_set ? 'Chave do OpenRouter configurada' : 'Sem chave do OpenRouter'}</span>
    </div>
    <p class="small muted" style="margin:12px 0">A chave de IA é configurada no servidor, na variável OPENROUTER_API_KEY.</p>

    {#if modelsErr}<p class="small" style="color:var(--bad)">{modelsErr}</p>{/if}
    {#if app.models}
      <ModelPicker label="Texto (receitas e descrições)" models={app.models.text} bind:value={s.text_model as string} />
      <ModelPicker label="Imagem (fotos dos drinks)" models={app.models.image} bind:value={s.image_model as string} />
      <ModelPicker label="Edição de imagem (fundo branco, melhorar fotos)" models={imageEditModels} bind:value={s.edit_model as string} />
    {:else if !modelsErr}
      <div class="skeleton" style="height:140px"></div>
    {/if}
  </div>

  <h2 class="sh">⛵ Navegação</h2>
  <div class="card pad">
    <div class="two">
      <label class="field">
        <span>Velocidade de cruzeiro (nós)</span>
        <input class="input" type="number" inputmode="decimal" step="0.5" min="1" bind:value={s.cruise_speed_kn} />
      </label>
      <label class="field">
        <span>Em movimento acima de (nós)</span>
        <input class="input" type="number" inputmode="decimal" step="0.1" min="0.2" bind:value={s.moving_threshold_kn} />
      </label>
    </div>
  </div>

  <button class="btn block" style="margin-top:14px" onclick={save} disabled={saving}>{saving ? 'Salvando…' : 'Salvar configurações'}</button>

  <h2 class="sh">🔔 Avisos neste aparelho</h2>
  <div class="card pad">
    <button class="toggle" onclick={() => { sound = !sound; setSoundEnabled(sound); if (sound) chime() }}>
      <span class="grow">Som de pedido novo</span>
      <span class="switch" class:on={sound}></span>
    </button>
    <div class="toggle">
      <span class="grow">Notificações push</span>
      {#if app.pushState === 'subscribed'}
        <span class="pill" style="color:var(--good)">ativas</span>
      {:else if !isStandalone()}
        <span class="small muted">instale na tela inicial</span>
      {:else if app.pushState === 'denied'}
        <span class="small muted">bloqueadas nos Ajustes do iPhone</span>
      {:else if app.pushState === 'unsupported'}
        <span class="small muted">não suportado</span>
      {:else}
        <button class="btn sm" onclick={() => app.enablePush()}>Ativar</button>
      {/if}
    </div>
  </div>

  <h2 class="sh">🧭 Tripulação</h2>
  <div class="card crew">
    {#each app.crewList as c (c.id)}
      <div class="crow">
        <span class="em">{c.emoji}</span>
        <span class="grow"><b>{c.name}</b>{#if c.id === app.crew?.id} <span class="small muted">(você)</span>{/if}</span>
        <button class="icon-btn" onclick={() => rename(c)} aria-label="Renomear"><Icon name="edit" size={18} /></button>
        <button class="icon-btn" onclick={() => removeCrew(c)} aria-label="Remover"><Icon name="trash" size={18} /></button>
      </div>
    {/each}
  </div>
  <p class="small muted" style="margin:8px 4px 0">Novos tripulantes entram tocando 4× no logo da tela inicial.</p>

  <h2 class="sh">🎨 Tema</h2>
  <div class="card pad">
    <p class="small muted" style="margin-bottom:10px">O tema muda sozinho com o horário. Para testar, veja como fica cada momento:</p>
    <div class="bands">
      <button class="chip" class:on={!app.bandPreview} onclick={() => app.previewBand(null)}>Automático</button>
      {#each BANDS as b}
        <button class="chip" class:on={app.bandPreview === b} onclick={() => app.previewBand(b)}>{BAND_INFO[b].nome}</button>
      {/each}
    </div>
    <p class="small muted" style="margin-top:10px">Manhã 5h–12h · Tarde 12h–17h · Final de tarde 17h–20h · Noite 20h–5h</p>
  </div>
</div>

<style>
  .me {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .em {
    font-size: 30px;
  }
  .me b {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 600;
  }
  .sh {
    font-size: 19px;
    margin: 26px 4px 10px;
  }
  .keyrow {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 800;
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--bad);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--bad) 20%, transparent);
  }
  .dot.ok {
    background: var(--good);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--good) 20%, transparent);
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 10px 0;
    text-align: left;
    font-weight: 700;
  }
  .toggle + .toggle {
    border-top: 1px solid var(--line);
  }
  .crew {
    padding: 4px 14px;
  }
  .crow {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .crow:last-child {
    border-bottom: 0;
  }
  .crow .icon-btn {
    width: 38px;
    height: 38px;
  }
  .bands {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
