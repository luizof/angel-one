<script lang="ts">
  // Garrafa com líquido ondulando. level: 0..1
  let { level = 1, size = 80, color = 'var(--accent)', still = false }: { level?: number; size?: number; color?: string; still?: boolean } = $props()
  const uid = Math.random().toString(36).slice(2, 8)
  const H = 100
  const top = 26 // onde começa o corpo
  const y = $derived(top + (1 - Math.max(0, Math.min(1, level))) * (H - top - 4))
</script>

<svg class="bottle" class:still viewBox="0 0 44 100" width={size * 0.44} height={size} aria-hidden="true">
  <defs>
    <clipPath id="b{uid}">
      <path d="M16 2h12v14c0 4 10 7 10 16v60a6 6 0 0 1-6 6H12a6 6 0 0 1-6-6V32c0-9 10-12 10-16z" />
    </clipPath>
  </defs>
  <g clip-path="url(#b{uid})">
    <rect x="0" y="0" width="44" height="100" fill="color-mix(in srgb, var(--ink) 6%, transparent)" />
    <g style="transform: translateY({y}px); transition: transform .7s var(--spring)">
      <path class="wave" d="M-44 0 q11 -4 22 0 t22 0 t22 0 t22 0 t22 0 V120 H-44z" fill={color} opacity=".55" />
      <path class="wave w2" d="M-44 2 q11 -4 22 0 t22 0 t22 0 t22 0 t22 0 V120 H-44z" fill={color} />
    </g>
    <rect x="11" y="34" width="4" height="50" rx="2" fill="#fff" opacity=".35" />
  </g>
  <path d="M16 2h12v14c0 4 10 7 10 16v60a6 6 0 0 1-6 6H12a6 6 0 0 1-6-6V32c0-9 10-12 10-16z" fill="none" stroke="var(--ink)" stroke-opacity=".35" stroke-width="2" />
  <rect x="15" y="0" width="14" height="6" rx="2" fill="var(--ink)" opacity=".5" />
</svg>

<style>
  .bottle {
    display: block;
    overflow: visible;
  }
  .wave {
    animation: slosh 2.4s linear infinite;
  }
  .w2 {
    animation-duration: 3.4s;
    animation-direction: reverse;
  }
  .still .wave {
    animation: none;
  }
  @keyframes slosh {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(44px);
    }
  }
</style>
