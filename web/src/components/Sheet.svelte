<script lang="ts">
  import type { Snippet } from 'svelte'
  import { fade } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'

  let {
    open = $bindable(false),
    title = '',
    full = false,
    hero,
    children,
    footer,
    onclose,
  }: {
    open?: boolean
    title?: string
    full?: boolean
    hero?: Snippet
    children?: Snippet
    footer?: Snippet
    onclose?: () => void
  } = $props()

  let dy = $state(0)
  let dragging = $state(false)
  let startY = 0
  let body: HTMLDivElement | undefined = $state()

  function close() {
    open = false
    onclose?.()
  }

  function down(e: PointerEvent) {
    if ((e.target as Element).closest('button, input, select, a')) return
    if (body && body.scrollTop > 0) return
    dragging = true
    startY = e.clientY
    ;(e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId)
  }
  function move(e: PointerEvent) {
    if (!dragging) return
    dy = Math.max(0, e.clientY - startY)
  }
  function up() {
    if (!dragging) return
    dragging = false
    if (dy > 110) close()
    dy = 0
  }

  function slide(_node: Element, { duration = 460 } = {}) {
    return {
      duration,
      easing: cubicOut,
      css: (t: number) => `transform: translateY(${(1 - t) * 105}%)`,
    }
  }

  $effect(() => {
    if (!open) return
    const prev = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && close()
    window.addEventListener('keydown', onKey)
    return () => {
      document.body.style.overflow = prev
      window.removeEventListener('keydown', onKey)
    }
  })
</script>

{#if open}
  <div class="backdrop" transition:fade={{ duration: 250 }} onclick={close} role="presentation"></div>
  <div
    class="sheet"
    class:full
    class:dragging
    style="--dy:{dy}px"
    transition:slide
    role="dialog"
    aria-modal="true"
    aria-label={title}
  >
    <div
      class="grab"
      onpointerdown={down}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
      role="presentation"
    >
      <span></span>
    </div>
    {#if hero}
      <div class="hero">{@render hero()}</div>
    {/if}
    {#if !title}
      <button class="icon-btn corner" onclick={close} aria-label="Fechar">✕</button>
    {/if}
    {#if title}
      <div class="head" onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} role="presentation">
        <h2>{title}</h2>
        <button class="icon-btn" onclick={close} aria-label="Fechar">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M6 6l12 12M18 6 6 18" /></svg>
        </button>
      </div>
    {/if}
    <div class="body" bind:this={body}>
      {@render children?.()}
    </div>
    {#if footer}
      <div class="foot">{@render footer()}</div>
    {/if}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 900;
    background: rgba(5, 15, 30, 0.42);
    -webkit-backdrop-filter: blur(3px);
    backdrop-filter: blur(3px);
  }
  .sheet {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 901;
    max-height: calc(100dvh - var(--safe-top) - 24px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border-radius: 30px 30px 0 0;
    box-shadow: 0 -20px 60px -20px rgba(0, 0, 0, 0.45);
    transform: translateY(var(--dy));
    transition: transform 0.35s var(--spring);
    max-width: 640px;
    margin: 0 auto;
    overflow: hidden;
  }
  .sheet.full {
    height: calc(100dvh - var(--safe-top) - 24px);
  }
  .sheet.dragging {
    transition: none;
  }
  .grab {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 26px;
    display: grid;
    place-items: center;
    z-index: 5;
    touch-action: none;
  }
  .grab span {
    width: 42px;
    height: 5px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--ink) 25%, transparent);
  }
  .hero {
    flex: none;
  }
  .corner {
    position: absolute;
    right: 12px;
    top: 12px;
    z-index: 6;
    background: var(--card-solid);
  }
  .head {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 26px 20px 8px;
    touch-action: none;
  }
  .head h2 {
    font-size: 24px;
  }
  .body {
    flex: 1;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    overscroll-behavior: contain;
    padding: 10px 20px 20px;
  }
  .foot {
    flex: none;
    padding: 12px 20px calc(14px + var(--safe-bottom));
    border-top: 1px solid var(--line);
    background: color-mix(in srgb, var(--bg) 92%, transparent);
  }
</style>
