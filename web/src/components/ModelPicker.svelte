<script lang="ts">
  // Lista suspensa de provedor seguida de lista suspensa de modelos (OpenRouter).
  import type { AiModel } from '../lib/types'

  let { models = [] as AiModel[], value = $bindable(''), label = '' }: { models?: AiModel[]; value?: string; label?: string } = $props()

  const providers = $derived([...new Set(models.map((m) => m.provider))].sort())
  let provider = $state('')

  $effect(() => {
    const p = value.split('/')[0]
    if (p && p !== provider && providers.includes(p)) provider = p
  })

  const list = $derived(models.filter((m) => m.provider === provider))

  function changeProvider(p: string) {
    provider = p
    const first = models.find((m) => m.provider === p)
    if (first && !value.startsWith(p + '/')) value = first.id
  }
</script>

<div class="mp">
  {#if label}<div class="label">{label}</div>{/if}
  <div class="row2">
    <select class="input" value={provider} onchange={(e) => changeProvider((e.target as HTMLSelectElement).value)}>
      {#if !providers.includes(provider)}<option value={provider}>{provider || '—'}</option>{/if}
      {#each providers as p}<option value={p}>{p}</option>{/each}
    </select>
    <select class="input" bind:value>
      {#if !list.some((m) => m.id === value)}<option value={value}>{value || '—'}</option>{/if}
      {#each list as m (m.id)}<option value={m.id}>{m.name.replace(/^[^:]+:\s*/, '')}</option>{/each}
    </select>
  </div>
</div>

<style>
  .mp {
    display: grid;
    gap: 6px;
    margin-bottom: 16px;
  }
  .row2 {
    display: grid;
    grid-template-columns: 0.8fr 1.2fr;
    gap: 8px;
  }
  .row2 .input {
    min-height: 46px;
    font-size: 15px;
    padding-right: 30px;
  }
</style>
