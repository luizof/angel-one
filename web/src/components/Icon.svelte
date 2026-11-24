<script lang="ts">
  let { name, size = 24, stroke = 2 }: { name: string; size?: number; stroke?: number } = $props()
</script>

<svg
  class="ic ic-{name}"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === 'home'}
    <!-- convés: barquinho com bandeira -->
    <path class="flag" d="M12 3v8M12 3l5 3-5 2" />
    <path d="M3 14h18l-2.6 4.2a3 3 0 0 1-2.55 1.4H8.15a3 3 0 0 1-2.55-1.4z" />
    <path class="water" d="M2 22c1.5 0 1.5-1 3-1s1.5 1 3 1 1.5-1 3-1 1.5 1 3 1 1.5-1 3-1 1.5 1 3 1" opacity=".55" />
  {:else if name === 'map'}
    <path d="M12 21s-7-6.5-7-11a7 7 0 0 1 14 0c0 4.5-7 11-7 11z" />
    <circle cx="12" cy="10" r="2.5" />
  {:else if name === 'bar'}
    <path d="M5 4h14l-7 8z" />
    <path d="M12 12v7M8 20h8" />
    <circle class="olive" cx="16.5" cy="3.5" r="1.6" />
    <path class="straw" d="M14 2l-2.5 7" />
  {:else if name === 'bell'}
    <path class="bell" d="M6 16V11a6 6 0 1 1 12 0v5l1.5 2h-15z" />
    <path d="M10 20.5a2 2 0 0 0 4 0" />
  {:else if name === 'box'}
    <path d="M3.5 8.5 12 4l8.5 4.5v7L12 20l-8.5-4.5z" />
    <path d="M3.5 8.5 12 13l8.5-4.5M12 13v7" />
    <path class="tape" d="M7.8 6.2l8.5 4.5" opacity=".6" />
  {:else if name === 'gear'}
    <circle cx="12" cy="12" r="3.2" />
    <path class="cog" d="M12 2.5v2.6M12 18.9v2.6M21.5 12h-2.6M5.1 12H2.5M18.7 5.3l-1.8 1.8M7.1 16.9l-1.8 1.8M18.7 18.7l-1.8-1.8M7.1 7.1 5.3 5.3" />
  {:else if name === 'plus'}
    <path d="M12 5v14M5 12h14" />
  {:else if name === 'close'}
    <path d="M6 6l12 12M18 6 6 18" />
  {:else if name === 'back'}
    <path d="M15 5l-7 7 7 7" />
  {:else if name === 'next'}
    <path d="M9 5l7 7-7 7" />
  {:else if name === 'edit'}
    <path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16z" />
    <path d="M13.5 6.5l4 4" />
  {:else if name === 'trash'}
    <path d="M4 7h16M10 11v6M14 11v6M6 7l1 12.5A1.6 1.6 0 0 0 8.6 21h6.8a1.6 1.6 0 0 0 1.6-1.5L18 7M9 7V4.5h6V7" />
  {:else if name === 'camera'}
    <path d="M4 8h3l1.8-2.5h6.4L17 8h3v11H4z" />
    <circle cx="12" cy="13" r="3.6" />
  {:else if name === 'sparkle'}
    <path d="M12 3c.6 4.2 2.8 6.4 7 7-4.2.6-6.4 2.8-7 7-.6-4.2-2.8-6.4-7-7 4.2-.6 6.4-2.8 7-7z" />
    <path d="M19 15.5c.25 1.6 1 2.3 2.5 2.5-1.5.25-2.25 1-2.5 2.5-.25-1.5-1-2.25-2.5-2.5 1.5-.2 2.25-.9 2.5-2.5z" />
  {:else if name === 'wind'}
    <path d="M3 9h11a3 3 0 1 0-3-3" />
    <path d="M3 13h15a3 3 0 1 1-3 3" />
    <path d="M3 17h7" />
  {:else if name === 'anchor'}
    <circle cx="12" cy="5" r="2" />
    <path d="M12 7v14M7.5 10.5h9M4 13a8 8 0 0 0 16 0" />
  {:else if name === 'compass'}
    <circle cx="12" cy="12" r="9" />
    <path d="M15.5 8.5l-2 5-5 2 2-5z" />
  {:else if name === 'share'}
    <path d="M12 15V3M8 7l4-4 4 4" />
    <path d="M6 11H5v10h14V11h-1" />
  {:else if name === 'check'}
    <path d="M4.5 12.5l5 5L19.5 7" />
  {:else if name === 'search'}
    <circle cx="11" cy="11" r="6.5" />
    <path d="M16 16l4.5 4.5" />
  {:else if name === 'locate'}
    <circle cx="12" cy="12" r="3.2" />
    <circle cx="12" cy="12" r="7.5" />
    <path d="M12 2v2.5M12 19.5V22M2 12h2.5M19.5 12H22" />
  {:else if name === 'wing'}
    <path d="M20 5c-5 0-11 2.5-14.5 7.5L4 15c2.5.5 5-.3 6.5-1.6-.6 1.8-.2 3.5 1 4.6 1.6-1.3 2.6-3.3 2.8-5.3.9 1.4 2.4 2 4 1.8-.4-1.8-1.6-3.1-3.2-3.6C18 9.3 19.6 7.3 20 5z" />
  {:else if name === 'list'}
    <path d="M9 6h11M9 12h11M9 18h11" />
    <path d="M4 6h.01M4 12h.01M4 18h.01" stroke-width="3" />
  {:else if name === 'cart'}
    <path d="M3 4h2.5l2 11h10.5l2-7.5H7" />
    <circle cx="9.5" cy="19.5" r="1.4" />
    <circle cx="17" cy="19.5" r="1.4" />
  {:else if name === 'clipboard'}
    <path d="M9 4h6v3H9z" />
    <path d="M15 5.5h3v15H6v-15h3" />
    <path d="M9 12l2 2 4-4" />
  {:else if name === 'minus'}
    <path d="M5 12h14" />
  {:else if name === 'boat'}
    <path d="M4 15h16l-2 4.5H6z" />
    <path d="M7 15l2-6h6.5l2 6" />
    <path d="M10.5 9V6h3" />
  {:else if name === 'sound'}
    <path d="M4 9.5h3.5L12 6v12l-4.5-3.5H4z" />
    <path class="waves" d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11" />
  {:else if name === 'logout'}
    <path d="M14 4h5v16h-5M10 8l-4 4 4 4M6 12h10" />
  {:else if name === 'history'}
    <path d="M3.5 12a8.5 8.5 0 1 0 2.5-6L3.5 8.5" />
    <path d="M3.5 4v4.5H8M12 7.5V12l3 2" />
  {:else if name === 'layers'}
    <path d="M12 3l9 5-9 5-9-5z" />
    <path d="M3 13l9 5 9-5" />
  {:else if name === 'refresh'}
    <path d="M20 11a8 8 0 1 0-2.3 5.7M20 4.5V11h-6.5" />
  {:else if name === 'tag'}
    <path d="M3.5 12.5V4h8.5l8.5 8.5-8.5 8.5z" />
    <circle cx="8" cy="8.5" r="1.5" />
  {:else}
    <circle cx="12" cy="12" r="8" />
  {/if}
</svg>

<style>
  .ic {
    flex: none;
    display: block;
  }
</style>
