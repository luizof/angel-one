<script lang="ts">
  import { app } from '../lib/store.svelte'
  import { forecastAhead, windColor } from '../lib/wind'
  import { DIR_NOME, dirOf, fmt1 } from '../lib/util'

  const f = $derived(forecastAhead(app.weather))
  const now = $derived(f.now)
  const kn = $derived(now?.wind_kn ?? 0)
  const strip = $derived(f.ahead.slice(0, 14))
  const maxKn = $derived(Math.max(15, ...strip.map((h) => h.gust_kn ?? h.wind_kn ?? 0)))
  const mood = $derived(
    kn < 5 ? 'Calmaria total' : kn < 10 ? 'Brisa gostosa' : kn < 16 ? 'Vento moderado' : kn < 22 ? 'Vento forte' : 'Ventania!',
  )
</script>

<div class="wind card" style="--speed:{Math.max(0.6, 6 - kn / 5)}s">
  <div class="streaks" aria-hidden="true">
    {#each Array(7) as _, i}<i style="top:{8 + i * 13}%; animation-delay:-{i * 0.7}s; width:{40 + ((i * 37) % 60)}px"></i>{/each}
  </div>
  {#if now && now.wind_kn != null}
    <div class="top">
      <div class="dial">
        <svg viewBox="-50 -50 100 100" width="96" height="96">
          <circle r="46" class="c1" />
          {#each ['N', 'L', 'S', 'O'] as l, i}
            <text x={Math.sin((i * Math.PI) / 2) * 36} y={-Math.cos((i * Math.PI) / 2) * 36} dy="0.35em">{l}</text>
          {/each}
          <g class="needle" style="transform: rotate({(now.wind_dir ?? 0) + 180}deg)">
            <path d="M0 -30 L8 -6 L2.5 -7 L2.5 26 L-2.5 26 L-2.5 -7 L-8 -6 Z" />
          </g>
          <circle r="4" class="dot" />
        </svg>
      </div>
      <div class="txt">
        <div class="label">Vento agora</div>
        <div class="big"><b>{fmt1(kn)}</b> <span>nós</span></div>
        <div class="desc">
          {#if now.wind_dir != null}de <b>{DIR_NOME[dirOf(now.wind_dir)]}</b>{/if}
          {#if now.gust_kn}· rajadas {fmt1(now.gust_kn)}{/if}
        </div>
        <div class="mood">{mood}{#if now.swell_m != null} · ondas {fmt1(now.swell_m)} m{/if}</div>
      </div>
    </div>
    <div class="strip">
      {#each strip as h, i}
        <div class="h" class:first={i === 0}>
          <div class="bar-wrap">
            <div class="bar" style="height:{((h.wind_kn ?? 0) / maxKn) * 100}%; background:{windColor(h.wind_kn)}"></div>
          </div>
          <svg class="arr" viewBox="-6 -6 12 12" width="12" height="12" style="transform: rotate({(h.wind_dir ?? 0) + 180}deg)">
            <path d="M0 -5 L4 3 L0 1 L-4 3 Z" />
          </svg>
          <div class="kn">{Math.round(h.wind_kn ?? 0)}</div>
          <div class="hr">{i === 0 ? 'agora' : h.time.slice(11, 13) + 'h'}</div>
        </div>
      {/each}
    </div>
  {:else if app.loaded}
    <div class="row"><span class="grow muted">Sem previsão atual para Jurerê.</span><button class="btn sm ghost" onclick={() => app.loadWeather()}>Atualizar</button></div>
  {:else}
    <div class="loading">
      <div class="skeleton" style="width:96px;height:96px;border-radius:50%"></div>
      <div class="grow">
        <div class="skeleton" style="height:16px;width:50%"></div>
        <div class="skeleton" style="height:30px;width:70%;margin-top:8px"></div>
      </div>
    </div>
  {/if}
</div>

<style>
  .wind {
    position: relative;
    overflow: hidden;
    padding: 16px 16px 12px;
  }
  .streaks i {
    position: absolute;
    right: -80px;
    height: 2px;
    border-radius: 2px;
    background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent2) 45%, transparent), transparent);
    animation: streak var(--speed) linear infinite;
    opacity: 0.6;
  }
  @keyframes streak {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(-560px);
    }
  }
  .top {
    position: relative;
    display: flex;
    gap: 16px;
    align-items: center;
  }
  .dial svg {
    display: block;
  }
  .c1 {
    fill: var(--card-solid);
    stroke: var(--line);
    stroke-width: 2;
  }
  text {
    font-family: var(--font-display);
    font-size: 11px;
    font-weight: 700;
    fill: var(--muted);
    text-anchor: middle;
  }
  .needle {
    transition: transform 1.4s var(--spring);
  }
  .needle path {
    fill: var(--accent);
  }
  .dot {
    fill: var(--ink);
  }
  .label {
    font-size: 12px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .big {
    font-family: var(--font-display);
    line-height: 1;
    margin: 3px 0 4px;
  }
  .big b {
    font-size: 40px;
    font-weight: 600;
  }
  .big span {
    font-size: 17px;
    color: var(--muted);
  }
  .desc {
    font-size: 15px;
  }
  .mood {
    font-size: 13px;
    color: var(--muted);
    margin-top: 2px;
  }
  .strip {
    position: relative;
    display: flex;
    gap: 4px;
    margin-top: 14px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .strip::-webkit-scrollbar {
    display: none;
  }
  .h {
    flex: 1 0 30px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
  }
  .bar-wrap {
    height: 38px;
    width: 9px;
    display: flex;
    align-items: flex-end;
    border-radius: 6px;
    background: var(--line);
    overflow: hidden;
  }
  .bar {
    width: 100%;
    border-radius: 6px;
    animation: grow 0.9s var(--spring) both;
    transform-origin: bottom;
  }
  @keyframes grow {
    from {
      transform: scaleY(0);
    }
  }
  .arr path {
    fill: var(--ink);
    opacity: 0.7;
  }
  .kn {
    font-weight: 800;
    font-size: 12px;
  }
  .hr {
    font-size: 10.5px;
    color: var(--muted);
    font-weight: 700;
  }
  .h.first .hr {
    color: var(--accent);
  }
  .loading {
    display: flex;
    gap: 16px;
    align-items: center;
  }
</style>
