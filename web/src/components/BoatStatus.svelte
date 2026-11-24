<script lang="ts">
  import { app } from '../lib/store.svelte'
  import { fmt1, timeAgo } from '../lib/util'

  const s = $derived(app.status)
  const known = $derived(s && s.lat != null && !s.stale)
</script>

{#if s && s.lat != null && (!known || s.speed_kn != null)}
  <div class="status" class:moving={known && s.moving} class:stale={!known} title="Atualizado {timeAgo(s.updated_at)}">
    {#if known && s.moving}
      <span class="wake" aria-hidden="true"><i></i><i></i><i></i></span>
      <span>Em movimento · {fmt1(s.speed_kn ?? 0)} nós</span>
    {:else if known}
      <span class="anchor" aria-hidden="true">⚓</span>
      <span>Parado</span>
    {:else}
      <span class="anchor" aria-hidden="true">📡</span>
      <span>Sem sinal {timeAgo(s.updated_at)}</span>
    {/if}
  </div>
{/if}

<style>
  .status {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 48px;
    padding: 0 15px 0 12px;
    border-radius: 99px;
    color: #fff;
    font-weight: 800;
    font-size: 13px;
    background: rgba(10, 25, 40, 0.28);
    border: 1px solid rgba(255, 255, 255, 0.25);
    -webkit-backdrop-filter: blur(14px) saturate(1.5);
    backdrop-filter: blur(14px) saturate(1.5);
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
  }
  .stale {
    opacity: 0.75;
  }
  .anchor {
    display: inline-block;
    animation: sway 4s ease-in-out infinite;
    transform-origin: 50% 0;
  }
  @keyframes sway {
    0%,
    100% {
      transform: rotate(-8deg);
    }
    50% {
      transform: rotate(8deg);
    }
  }
  .wake {
    display: inline-flex;
    flex-direction: column;
    gap: 2px;
  }
  .wake i {
    display: block;
    height: 2px;
    width: 14px;
    border-radius: 2px;
    background: #fff;
    animation: wake 0.9s linear infinite;
  }
  .wake i:nth-child(2) {
    animation-delay: 0.3s;
    width: 10px;
  }
  .wake i:nth-child(3) {
    animation-delay: 0.6s;
  }
  @keyframes wake {
    from {
      transform: translateX(6px);
      opacity: 0;
    }
    50% {
      opacity: 1;
    }
    to {
      transform: translateX(-6px);
      opacity: 0;
    }
  }
  .moving {
    background: color-mix(in srgb, var(--accent) 55%, rgba(0, 0, 0, 0.2));
  }
</style>
