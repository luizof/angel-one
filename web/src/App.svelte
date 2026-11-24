<script lang="ts">
  import { onMount } from 'svelte'
  import { app } from './lib/store.svelte'
  import { router } from './lib/router.svelte'
  import { unlock } from './lib/sound'
  import TabBar from './components/TabBar.svelte'
  import Toasts from './components/Toasts.svelte'
  import CrewSheet from './components/CrewSheet.svelte'
  import Home from './views/Home.svelte'
  import Destinos from './views/Destinos.svelte'
  import Bar from './views/Bar.svelte'
  import Pedidos from './views/Pedidos.svelte'
  import Despensa from './views/Despensa.svelte'
  import Ajustes from './views/Ajustes.svelte'

  onMount(() => {
    app.init()
    app.checkPush().catch(() => {})
    const first = () => {
      unlock()
      window.removeEventListener('pointerdown', first)
    }
    window.addEventListener('pointerdown', first)
  })

  const section = $derived(router.section)
  const hideTabs = $derived(router.path.startsWith('/despensa/conferir'))

  // rotas só da tripulação voltam para o início se o aparelho sair do modo
  $effect(() => {
    if (!app.isCrew && (section === '/pedidos' || section === '/ajustes')) router.go('/', true)
  })
</script>

{#key section}
<main class="view">
  {#if section === '/'}
    <Home />
  {:else if section === '/destinos'}
    <Destinos />
  {:else if section === '/bar'}
    <Bar />
  {:else if section === '/pedidos'}
    <Pedidos />
  {:else if section === '/despensa'}
    <Despensa />
  {:else if section === '/ajustes'}
    <Ajustes />
  {:else}
    <Home />
  {/if}
</main>
{/key}

{#if !hideTabs}<TabBar />{/if}
<Toasts />
<CrewSheet />

{#if !app.online && app.loaded}
  <div class="offline">Reconectando ao barco…</div>
{/if}

<style>
  .view {
    min-height: 100dvh;
    animation: arrive .28s var(--ease-out);
  }
  :global(html[data-navdir='back']) .view { --from: -12px }
  @keyframes arrive {
    from { opacity: .65; transform: translateX(var(--from, 12px)); }
    to { opacity: 1; transform: none; }
  }
  .offline {
    position: fixed;
    left: 50%;
    bottom: calc(var(--tabbar-h) + var(--safe-bottom) + 18px);
    transform: translateX(-50%);
    z-index: 600;
    padding: 7px 14px;
    border-radius: 99px;
    font-size: 13px;
    font-weight: 800;
    color: #fff;
    background: rgba(20, 30, 50, 0.75);
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
    animation: rise 0.4s var(--ease-out);
  }
</style>
