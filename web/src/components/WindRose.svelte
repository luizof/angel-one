<script lang="ts">
  // Rosa dos ventos com 8 setores. Editável (escolher "bom para ventos")
  // ou só leitura (mostra setores abrigados + seta do vento atual).
  import { DIRS, type Dir } from '../lib/types'
  import { pop } from '../lib/sound'

  let {
    value = $bindable([] as Dir[]),
    editable = false,
    wind = null as number | null,
    size = 260,
  }: { value?: Dir[]; editable?: boolean; wind?: number | null; size?: number } = $props()

  const R = 100
  const r0 = 34

  function wedge(i: number) {
    const a0 = ((i * 45 - 22.5 - 90) * Math.PI) / 180
    const a1 = ((i * 45 + 22.5 - 90) * Math.PI) / 180
    const p = (r: number, a: number) => `${(r * Math.cos(a)).toFixed(2)} ${(r * Math.sin(a)).toFixed(2)}`
    return `M ${p(r0, a0)} L ${p(R, a0)} A ${R} ${R} 0 0 1 ${p(R, a1)} L ${p(r0, a1)} A ${r0} ${r0} 0 0 0 ${p(r0, a0)} Z`
  }
  function labelPos(i: number) {
    const a = ((i * 45 - 90) * Math.PI) / 180
    const r = (R + r0) / 2
    return { x: r * Math.cos(a), y: r * Math.sin(a) }
  }
  function toggle(d: Dir) {
    if (!editable) return
    pop()
    value = value.includes(d) ? value.filter((x) => x !== d) : [...value, d]
  }
</script>

<svg class="rose" class:editable viewBox="-112 -112 224 224" width={size} height={size} role="group" aria-label="Rosa dos ventos">
  <circle r="108" class="ring" />
  {#each DIRS as d, i}
    <g class="sector" class:on={value.includes(d)} onclick={() => toggle(d)} role="button" tabindex="-1" aria-pressed={value.includes(d)} onkeydown={() => {}}>
      <path d={wedge(i)} />
      <text x={labelPos(i).x} y={labelPos(i).y} dy="0.35em">{d}</text>
    </g>
  {/each}
  <circle r={r0 - 6} class="hub" />
  {#if wind != null}
    <g class="arrow" style="transform: rotate({wind + 180}deg)">
      <path d="M0 -26 L7 -8 L2 -9 L2 24 L-2 24 L-2 -9 L-7 -8 Z" />
    </g>
  {:else}
    <text class="hubtxt" dy="0.35em">🪽</text>
  {/if}
</svg>

<style>
  .rose {
    display: block;
    margin: 0 auto;
    overflow: visible;
    touch-action: manipulation;
  }
  .ring {
    fill: none;
    stroke: var(--line);
    stroke-width: 1.5;
    stroke-dasharray: 2 5;
  }
  .sector path {
    fill: var(--card);
    stroke: var(--bg);
    stroke-width: 3;
    transition: fill 0.3s, transform 0.4s var(--spring);
    transform-origin: 0 0;
  }
  .sector text {
    fill: var(--muted);
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 15px;
    text-anchor: middle;
    pointer-events: none;
    transition: fill 0.3s;
  }
  .editable .sector {
    cursor: pointer;
  }
  .editable .sector:active path {
    transform: scale(0.95);
  }
  .sector.on path {
    fill: var(--accent);
    transform: scale(1.04);
  }
  .sector.on text {
    fill: var(--accent-ink);
  }
  .hub {
    fill: var(--card-solid);
    stroke: var(--line);
  }
  .hubtxt {
    text-anchor: middle;
    font-size: 22px;
  }
  .arrow {
    transition: transform 1s var(--spring);
  }
  .arrow path {
    fill: var(--ink);
  }
</style>
