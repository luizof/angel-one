<script lang="ts">
  import { app } from '../lib/store.svelte'
  import { fly } from 'svelte/transition'
  import { flip } from 'svelte/animate'
</script>

<div class="toasts">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}" in:fly={{ y: -30, duration: 400 }} out:fly={{ y: -20, duration: 250 }} animate:flip={{ duration: 300 }}>
      {#if t.icon}<span class="i">{t.icon}</span>{/if}
      <span>{t.text}</span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    top: calc(var(--safe-top) + 10px);
    left: 12px;
    right: 12px;
    z-index: 2000;
    display: grid;
    gap: 8px;
    pointer-events: none;
    justify-items: center;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 460px;
    padding: 12px 16px;
    border-radius: 18px;
    background: color-mix(in srgb, var(--card-solid) 88%, transparent);
    border: 1px solid var(--line);
    box-shadow: 0 16px 34px -14px rgba(0, 0, 0, 0.45);
    -webkit-backdrop-filter: blur(16px);
    backdrop-filter: blur(16px);
    font-weight: 700;
    font-size: 15px;
  }
  .toast.err {
    border-color: color-mix(in srgb, var(--bad) 50%, transparent);
  }
  .i {
    font-size: 20px;
  }
</style>
