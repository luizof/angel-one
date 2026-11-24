<script lang="ts">
  // Logo animado: asas batendo devagar + auréola flutuando.
  // Toque 4× rápido = entrada secreta da tripulação.
  let {
    size = 120,
    showName = true,
    onsecret,
  }: { size?: number; showName?: boolean; onsecret?: () => void } = $props()

  let taps = $state(0)
  let flutter = $state(false)
  let timer = 0
  let el: HTMLButtonElement

  function tap() {
    taps++
    flutter = false
    requestAnimationFrame(() => (flutter = true))
    clearTimeout(timer)
    timer = window.setTimeout(() => (taps = 0), 1300)
    if (taps >= 4) {
      taps = 0
      onsecret?.()
    }
  }
</script>

<button
  bind:this={el}
  class="logo"
  class:flutter
  style="--w:{size}px; --glow-k:{1 + taps * 0.35}"
  onclick={tap}
  onanimationend={() => (flutter = false)}
  aria-label="Angel One"
>
  <span class="halo-wrap"><span class="halo"></span></span>
  <span class="wings">
    <img class="wing l" src="/img/asa-esq.png" alt="" draggable="false" />
    <img class="wing r" src="/img/asa-dir.png" alt="" draggable="false" />
  </span>
  {#if showName}<span class="name">Angel One</span>{/if}
</button>

<style>
  .logo {
    touch-action: manipulation;
    position: relative;
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    animation: float 5.5s ease-in-out infinite;
    filter: drop-shadow(0 6px 14px rgba(0, 0, 0, 0.28));
  }
  .wings {
    position: relative;
    display: flex;
    justify-content: center;
    gap: calc(var(--w) * 0.06);
    width: var(--w);
    height: calc(var(--w) * 0.63);
  }
  .wing {
    width: calc(var(--w) * 0.47);
    height: auto;
    pointer-events: none;
    -webkit-user-drag: none;
  }
  .wing.l {
    transform-origin: 96% 30%;
    animation: flap-l 3.6s ease-in-out infinite;
  }
  .wing.r {
    transform-origin: 4% 30%;
    animation: flap-r 3.6s ease-in-out infinite;
  }
  .flutter .wing.l {
    animation: flutter-l 0.55s ease-in-out 1;
  }
  .flutter .wing.r {
    animation: flutter-r 0.55s ease-in-out 1;
  }
  .halo-wrap {
    position: absolute;
    top: calc(var(--w) * -0.2);
    left: 50%;
    width: calc(var(--w) * 0.42);
    height: calc(var(--w) * 0.11);
    transform: translateX(-50%);
    animation: halo-bob 3.6s ease-in-out infinite;
    z-index: 2;
  }
  .halo {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: 50%;
    border: calc(var(--w) * 0.028) solid #ffd36b;
    box-shadow:
      0 0 calc(10px * var(--glow-k)) rgba(255, 214, 110, 0.95),
      0 0 calc(26px * var(--glow-k)) rgba(255, 200, 90, 0.7),
      inset 0 0 8px rgba(255, 240, 200, 0.9);
    transition: box-shadow 0.3s;
    animation: halo-shine 2.8s ease-in-out infinite;
  }
  .name {
    font-family: var(--font-script);
    font-size: calc(var(--w) * 0.36);
    line-height: 1;
    color: #fff;
    margin-top: calc(var(--w) * -0.02);
    text-shadow:
      0 2px 12px rgba(0, 0, 0, 0.35),
      0 0 22px rgba(255, 230, 170, 0.35);
    white-space: nowrap;
  }
  @keyframes float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-6px);
    }
  }
  @keyframes flap-l {
    0%,
    100% {
      transform: rotate(0deg);
    }
    45% {
      transform: rotate(-9deg) scaleX(0.96);
    }
  }
  @keyframes flap-r {
    0%,
    100% {
      transform: rotate(0deg);
    }
    45% {
      transform: rotate(9deg) scaleX(0.96);
    }
  }
  @keyframes flutter-l {
    0%,
    100% {
      transform: rotate(0);
    }
    30% {
      transform: rotate(-24deg) scaleX(0.9);
    }
    65% {
      transform: rotate(6deg);
    }
  }
  @keyframes flutter-r {
    0%,
    100% {
      transform: rotate(0);
    }
    30% {
      transform: rotate(24deg) scaleX(0.9);
    }
    65% {
      transform: rotate(-6deg);
    }
  }
  @keyframes halo-bob {
    0%,
    100% {
      transform: translateX(-50%) translateY(0) rotate(-2deg);
    }
    50% {
      transform: translateX(-50%) translateY(-5px) rotate(2deg);
    }
  }
  @keyframes halo-shine {
    0%,
    100% {
      filter: brightness(1);
    }
    50% {
      filter: brightness(1.25);
    }
  }
</style>
