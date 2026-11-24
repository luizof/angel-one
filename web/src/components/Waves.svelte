<script lang="ts">
  let { height = 70 }: { height?: number } = $props()
  const d = 'M0 30 C 150 0 350 0 500 30 C 650 60 850 60 1000 30 C 1150 0 1350 0 1500 30 C 1650 60 1850 60 2000 30 V 80 H 0 Z'
</script>

<div class="waves" style="height:{height}px" aria-hidden="true">
  <svg class="w w3" viewBox="0 0 2000 80" preserveAspectRatio="none"><path {d} /></svg>
  <svg class="w w2" viewBox="0 0 2000 80" preserveAspectRatio="none"><path {d} /></svg>
  <svg class="w w1" viewBox="0 0 2000 80" preserveAspectRatio="none"><path {d} /></svg>
</div>

<style>
  .waves {
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    overflow: hidden;
    pointer-events: none;
  }
  .w {
    position: absolute;
    bottom: 0;
    left: 0;
    width: 200%;
    height: 100%;
  }
  .w1 {
    fill: var(--bg);
    height: 62%;
    animation: drift 11s linear infinite;
    transition: fill 1.2s;
  }
  .w2 {
    fill: var(--wave1);
    height: 82%;
    animation: drift 16s linear infinite reverse;
    opacity: 0.9;
  }
  .w3 {
    fill: var(--wave2);
    height: 100%;
    animation: drift 23s linear infinite;
    opacity: 0.8;
  }
  @keyframes drift {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(-50%);
    }
  }
</style>
