<script lang="ts">
  import Icon from './Icon.svelte'
  import { router } from '../lib/router.svelte'
  import { app } from '../lib/store.svelte'
  import { pop } from '../lib/sound'

  const pending = $derived(app.orders.filter((o) => o.status === 'recebido' || o.status === 'preparando').length)

  const tabs = $derived([
    { path: '/', label: 'Convés', icon: 'home' },
    { path: '/destinos', label: 'Destinos', icon: 'map' },
    { path: '/bar', label: 'Bar', icon: 'bar' },
    ...(app.isCrew ? [{ path: '/pedidos', label: 'Comandas', icon: 'bell', badge: pending }] : []),
    { path: '/despensa', label: 'Despensa', icon: 'box' },
  ] as { path: string; label: string; icon: string; badge?: number }[])

  const activeIndex = $derived(Math.max(0, tabs.findIndex((t) => t.path === router.section)))
</script>

<nav class="tabbar" style="--n:{tabs.length}; --i:{activeIndex}">
  <div class="glass">
    <span class="halo" aria-hidden="true"></span>
    {#each tabs as t (t.path)}
      <button
        class="tab"
        class:on={router.section === t.path}
        onclick={() => {
          pop()
          router.go(t.path)
        }}
        aria-label={t.label}
      >
        <span class="ico"><Icon name={t.icon} size={25} stroke={2.1} /></span>
        <span class="lbl">{t.label}</span>
        {#if t.badge}
          <span class="badge">{t.badge}</span>
        {/if}
      </button>
    {/each}
  </div>
</nav>

<style>
  .tabbar {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 500;
    padding: 0 14px calc(var(--safe-bottom) + 10px);
    pointer-events: none;
    view-transition-name: tabbar;
  }
  .glass {
    pointer-events: auto;
    position: relative;
    display: grid;
    grid-template-columns: repeat(var(--n), 1fr);
    max-width: 520px;
    margin: 0 auto;
    height: 66px;
    border-radius: 26px;
    background: color-mix(in srgb, var(--card-solid) 72%, transparent);
    border: 1px solid var(--line);
    box-shadow: 0 18px 40px -16px rgba(0, 0, 0, 0.35);
    -webkit-backdrop-filter: blur(22px) saturate(1.6);
    backdrop-filter: blur(22px) saturate(1.6);
  }
  .halo {
    position: absolute;
    top: 5px;
    left: calc((100% / var(--n)) * var(--i) + (100% / var(--n)) / 2);
    width: 26px;
    height: 7px;
    transform: translateX(-50%);
    border-radius: 50%;
    border: 2px solid var(--gold);
    box-shadow: 0 0 8px var(--gold), 0 0 16px color-mix(in srgb, var(--gold) 60%, transparent);
    transition: left 0.5s var(--spring);
    animation: halo 2.6s ease-in-out infinite;
  }
  @keyframes halo {
    0%,
    100% {
      transform: translateX(-50%) translateY(0);
    }
    50% {
      transform: translateX(-50%) translateY(-2px);
    }
  }
  .tab {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    color: var(--muted);
    transition: color 0.25s;
  }
  .tab .ico {
    transition: transform 0.45s var(--spring);
  }
  .tab.on {
    color: var(--accent);
  }
  .tab.on .ico {
    transform: translateY(1px) scale(1.12);
    animation: wiggle 0.6s ease-in-out;
  }
  .tab:active .ico {
    transform: scale(0.85);
  }
  .lbl {
    font-size: 10.5px;
    font-weight: 800;
    letter-spacing: 0.02em;
  }
  .badge {
    position: absolute;
    top: 7px;
    left: calc(50% + 6px);
    min-width: 19px;
    height: 19px;
    padding: 0 5px;
    border-radius: 99px;
    display: grid;
    place-items: center;
    background: var(--bad);
    color: #fff;
    font-size: 11px;
    font-weight: 900;
    animation: pop-in 0.5s var(--spring);
    box-shadow: 0 0 0 2px var(--card-solid);
  }
</style>
