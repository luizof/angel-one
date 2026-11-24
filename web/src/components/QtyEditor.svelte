<script lang="ts">
  // Editor de quantidade: itens fracionados = garrafas cheias + nível da aberta;
  // demais = botões +/−.
  import Bottle from './Bottle.svelte'
  import { fmt2 } from '../lib/util'
  import { pop } from '../lib/sound'

  let {
    value = $bindable(0),
    fractional = false,
    unit = 'un',
    step = 1,
  }: { value?: number; fractional?: boolean; unit?: string; step?: number } = $props()

  const LEVELS = [0, 0.25, 0.5, 0.75]
  const LBL = ['vazia', '¼', '½', '¾']

  const full = $derived(Math.floor(value + 1e-9))
  const part = $derived(Math.round((value - full) * 4) / 4)

  function setFull(n: number) {
    pop()
    value = Math.max(0, n) + part
  }
  function setPart(p: number) {
    pop()
    value = full + p
  }
  function inc(d: number) {
    pop()
    value = Math.max(0, Math.round((value + d) * 1000) / 1000)
  }
</script>

{#if fractional}
  <div class="frac">
    <div class="block">
      <div class="label">Fechadas / cheias</div>
      <div class="stepper">
        <button class="sbtn" onclick={() => setFull(full - 1)} disabled={full <= 0} aria-label="menos">−</button>
        <div class="num"><b>{full}</b></div>
        <button class="sbtn" onclick={() => setFull(full + 1)} aria-label="mais">+</button>
      </div>
      <div class="shelf">
        {#each Array(Math.min(full, 8)) as _, i}<span style="animation-delay:{i * 0.04}s"><Bottle level={1} size={44} still /></span>{/each}
        {#if full > 8}<span class="more">+{full - 8}</span>{/if}
      </div>
    </div>
    <div class="block">
      <div class="label">Garrafa aberta</div>
      <div class="open">
        <Bottle level={part} size={110} />
        <div class="levels">
          {#each LEVELS as l, i}
            <button class="lv" class:on={part === l} onclick={() => setPart(l)}>{LBL[i]}</button>
          {/each}
        </div>
      </div>
    </div>
    <div class="total">Total: <b>{fmt2(value)}</b> {unit}{value === 1 ? '' : 's'}</div>
  </div>
{:else}
  <div class="stepper big">
    <button class="sbtn" onclick={() => inc(-step)} disabled={value <= 0} aria-label="menos">−</button>
    <div class="num">
      <input type="number" inputmode="decimal" step="any" min="0" bind:value />
      <small>{unit}</small>
    </div>
    <button class="sbtn" onclick={() => inc(step)} aria-label="mais">+</button>
  </div>
{/if}

<style>
  .frac {
    display: grid;
    gap: 14px;
  }
  .block {
    padding: 14px;
    border-radius: 20px;
    background: var(--card);
    border: 1px solid var(--line);
  }
  .label {
    margin-bottom: 8px;
  }
  .stepper {
    display: flex;
    align-items: center;
    gap: 14px;
    justify-content: center;
  }
  .sbtn {
    width: 52px;
    height: 52px;
    border-radius: 50%;
    font-size: 28px;
    font-weight: 700;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
    transition: transform 0.25s var(--spring);
  }
  .sbtn:active {
    transform: scale(0.85);
  }
  .sbtn[disabled] {
    opacity: 0.35;
  }
  .num {
    min-width: 90px;
    text-align: center;
    font-family: var(--font-display);
  }
  .num b {
    font-size: 40px;
    font-weight: 600;
  }
  .big .sbtn {
    width: 64px;
    height: 64px;
    font-size: 34px;
  }
  .big .num input {
    width: 120px;
    border: 0;
    background: none;
    text-align: center;
    font-family: var(--font-display);
    font-size: 46px;
    font-weight: 600;
    outline: none;
  }
  .big .num small {
    display: block;
    color: var(--muted);
    font-weight: 700;
  }
  .shelf {
    display: flex;
    justify-content: center;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 10px;
    min-height: 44px;
  }
  .shelf span {
    animation: pop-in 0.4s var(--spring) both;
  }
  .more {
    align-self: center;
    font-weight: 900;
    color: var(--muted);
  }
  .open {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 22px;
  }
  .levels {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .lv {
    min-width: 64px;
    height: 46px;
    border-radius: 14px;
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 600;
    background: var(--card-solid);
    border: 1.5px solid var(--line);
    transition: transform 0.25s var(--spring), background 0.2s;
  }
  .lv.on {
    background: var(--accent);
    color: var(--accent-ink);
    border-color: transparent;
    transform: scale(1.05);
  }
  .total {
    text-align: center;
    color: var(--muted);
  }
  .total b {
    color: var(--ink);
    font-size: 18px;
  }
</style>
