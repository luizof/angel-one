<script lang="ts">
  // Ilhota low poly que muda com o horário (sol, pôr do sol, lua e estrelas).
  import { app } from '../lib/store.svelte'
  const night = $derived(app.activeBand === 'noite')
  const dusk = $derived(app.activeBand === 'final_de_tarde')
</script>

<svg class="island" viewBox="0 0 360 170" aria-hidden="true">
  <!-- astro -->
  {#if night}
    <g class="stars">
      {#each [[40, 30], [90, 18], [140, 42], [220, 20], [300, 36], [330, 14], [260, 50], [60, 60]] as [x, y], i}
        <circle cx={x} cy={y} r={i % 3 === 0 ? 1.8 : 1.2} style="animation-delay:{i * 0.4}s" />
      {/each}
    </g>
    <g class="moon"><circle cx="285" cy="52" r="16" fill="#fff4cf" /><circle cx="292" cy="46" r="14" fill="var(--bg)" /></g>
  {:else}
    <g class="sun" style="transform-origin: 280px {dusk ? 98 : 52}px">
      <circle cx="280" cy={dusk ? 98 : 52} r={dusk ? 24 : 18} fill={dusk ? '#ff8a4c' : '#ffd166'} />
      {#each Array(10) as _, i}
        <rect x="278.5" y={dusk ? 62 : 22} width="3" height="8" rx="1.5" fill={dusk ? '#ffb07a' : '#ffd166'} transform="rotate({i * 36} 280 {dusk ? 98 : 52})" />
      {/each}
    </g>
  {/if}

  <!-- mar low poly -->
  <g class="sea">
    <polygon points="0,120 60,112 120,122 180,110 240,121 300,111 360,120 360,170 0,170" fill="var(--accent2)" opacity=".35" />
    <polygon points="0,132 70,124 140,134 210,123 280,133 360,124 360,170 0,170" fill="var(--accent)" opacity=".45" />
    <polygon points="0,146 50,140 110,148 170,139 230,147 300,139 360,146 360,170 0,170" fill="var(--accent)" opacity=".7" />
  </g>

  <!-- ilha -->
  <g class="land">
    <polygon points="110,124 150,96 196,90 236,102 262,124" fill="#f4d79b" />
    <polygon points="150,96 196,90 176,112" fill="#e8c27c" />
    <polygon points="196,90 236,102 210,116" fill="#f9e4b4" />
    <polygon points="118,124 150,96 160,124" fill="#e6bd73" />
  </g>

  <!-- coqueiro -->
  <g class="palm">
    <polygon points="190,92 186,92 178,52 182,51" fill="#9b6a3a" />
    <polygon points="186,92 183,92 176,52 178,52" fill="#80532b" />
    <g class="fronds" style="transform-origin: 180px 50px">
      <polygon points="180,50 150,46 132,58 158,52" fill="#2fa86b" />
      <polygon points="180,50 158,52 140,70 164,58" fill="#258c58" />
      <polygon points="180,50 206,40 226,52 202,50" fill="#33b874" />
      <polygon points="180,50 202,50 220,68 198,58" fill="#258c58" />
      <polygon points="180,50 170,30 182,22 186,40" fill="#3cc47d" />
      <polygon points="180,50 192,32 210,30 192,44" fill="#2fa86b" />
      <circle cx="178" cy="55" r="3" fill="#6b4a22" />
      <circle cx="184" cy="56" r="3" fill="#5a3d1c" />
    </g>
  </g>

  <!-- pena flutuando -->
  <g class="feather">
    <path d="M70 70 c6 -4 10 4 6 12 c-6 -2 -9 -6 -6 -12z" fill="#fff" stroke="var(--gold)" stroke-width="1" />
  </g>
</svg>

<style>
  .island {
    display: block;
    width: 100%;
    max-width: 420px;
    margin: 0 auto;
  }
  .sun {
    animation: spin 40s linear infinite;
  }
  .stars circle {
    fill: #fff;
    animation: twinkle 2.6s ease-in-out infinite;
  }
  @keyframes twinkle {
    0%,
    100% {
      opacity: 0.25;
    }
    50% {
      opacity: 1;
    }
  }
  .sea polygon {
    animation: seasway 6s ease-in-out infinite;
  }
  .sea polygon:nth-child(2) {
    animation-duration: 7.5s;
    animation-direction: reverse;
  }
  @keyframes seasway {
    0%,
    100% {
      transform: translateX(0);
    }
    50% {
      transform: translateX(-8px);
    }
  }
  .fronds {
    animation: palm 4.5s ease-in-out infinite;
  }
  @keyframes palm {
    0%,
    100% {
      transform: rotate(-3deg);
    }
    50% {
      transform: rotate(4deg);
    }
  }
  .feather {
    animation: featherfloat 9s ease-in-out infinite;
  }
  @keyframes featherfloat {
    0% {
      transform: translate(0, 0) rotate(0);
      opacity: 0;
    }
    15% {
      opacity: 1;
    }
    50% {
      transform: translate(40px, 20px) rotate(30deg);
    }
    85% {
      opacity: 1;
    }
    100% {
      transform: translate(80px, 40px) rotate(-10deg);
      opacity: 0;
    }
  }
</style>
