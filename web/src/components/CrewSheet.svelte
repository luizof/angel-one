<script lang="ts">
  import Sheet from './Sheet.svelte'
  import { app } from '../lib/store.svelte'
  import { api } from '../lib/api'
  import type { Crew } from '../lib/types'
  import { burst } from '../lib/fx'
  import { sparkle } from '../lib/sound'

  const EMOJIS = ['🧭', '⚓', '🐬', '🦈', '🐢', '🦀', '🐙', '🌴', '☀️', '🍹', '🎣', '🪽', '🏄', '🌊', '⭐', '🦜']

  let novo = $state(false)
  let name = $state('')
  let emoji = $state('🧭')
  let busy = $state(false)

  $effect(() => {
    if (app.crewSheet) {
      app.loadCrew()
      novo = false
      name = ''
    }
  })

  function enter(c: Crew) {
    app.becomeCrew(c)
    app.crewSheet = false
    sparkle()
    burst('feathers', innerWidth / 2, innerHeight * 0.3, 22)
    app.toast(`Bem-vindo à tripulação, ${c.name}!`, 'ok', '🪽')
  }

  async function create() {
    if (!name.trim()) return
    busy = true
    try {
      const c = await api<Crew>('/api/crew', { body: { name: name.trim(), emoji } })
      enter(c)
    } catch (e) {
      app.fail(e)
    } finally {
      busy = false
    }
  }
</script>

<Sheet bind:open={app.crewSheet} title="">
  <div class="welcome">
    <div class="halo"></div>
    <h2>Área da tripulação</h2>
    <p class="muted">Quem está no comando hoje?</p>
  </div>

  {#if !novo}
    <div class="list stagger">
      {#each app.crewList as c (c.id)}
        <button class="crew" onclick={() => enter(c)}>
          <span class="em">{c.emoji || '🧭'}</span>
          <span class="nm">{c.name}</span>
          <span class="go">→</span>
        </button>
      {/each}
    </div>
    <button class="btn soft block" style="margin-top:14px" onclick={() => (novo = true)}>+ Sou novo na tripulação</button>
  {:else}
    <label class="field">
      <span>Seu nome</span>
      <input class="input" bind:value={name} placeholder="Ex.: Luiz" maxlength="30" autocomplete="given-name" />
    </label>
    <div class="label" style="margin-bottom:8px">Seu símbolo</div>
    <div class="emojis">
      {#each EMOJIS as e}
        <button class="emo" class:on={emoji === e} onclick={() => (emoji = e)}>{e}</button>
      {/each}
    </div>
    <div class="row" style="margin-top:18px">
      <button class="btn ghost" onclick={() => (novo = false)}>Voltar</button>
      <button class="btn grow" disabled={!name.trim() || busy} onclick={create}>Entrar a bordo</button>
    </div>
  {/if}
</Sheet>

<style>
  .welcome {
    text-align: center;
    padding: 18px 0 18px;
  }
  .welcome h2 {
    font-size: 26px;
    margin-top: 14px;
  }
  .welcome p {
    margin-top: 4px;
  }
  .halo {
    width: 64px;
    height: 16px;
    margin: 0 auto;
    border-radius: 50%;
    border: 4px solid var(--gold);
    box-shadow: 0 0 14px var(--gold), 0 0 30px color-mix(in srgb, var(--gold) 60%, transparent);
    animation: bob 3s ease-in-out infinite;
  }
  .list {
    display: grid;
    gap: 10px;
  }
  .crew {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    border-radius: 18px;
    background: var(--card);
    border: 1px solid var(--line);
    text-align: left;
    transition: transform 0.25s var(--spring);
  }
  .crew:active {
    transform: scale(0.97);
  }
  .em {
    font-size: 28px;
  }
  .nm {
    flex: 1;
    font-family: var(--font-display);
    font-size: 19px;
    font-weight: 600;
  }
  .go {
    color: var(--accent);
    font-weight: 900;
  }
  .emojis {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 6px;
  }
  .emo {
    aspect-ratio: 1;
    border-radius: 14px;
    font-size: 22px;
    background: var(--card);
    border: 1.5px solid var(--line);
    transition: transform 0.25s var(--spring);
  }
  .emo.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    transform: scale(1.08);
  }
</style>
