<script lang="ts">
  import { ctl } from './controller.svelte';
  import NumberField from './NumberField.svelte';

  let { output }: { output: number } = $props();
  const comp = $derived(ctl.memory.output(output).compressor);
</script>

<section class="panel">
  <div class="panel-head">
    <h2>Compressor</h2>
    <button class="warn" class:on={comp.bypass} aria-pressed={comp.bypass ?? false} onclick={() => ctl.setCompressor(output, { bypass: !comp.bypass })}>
      {comp.bypass === undefined ? 'Bypass ?' : 'Bypass'}
    </button>
  </div>
  <div class="panel-body grid">
    <NumberField label="Threshold" value={comp.threshold} min={-90} max={0} step={0.5} digits={1} unit="dBFS" onset={(v) => ctl.setCompressor(output, { threshold: v })} />
    <NumberField label="Ratio" value={comp.ratio} min={1} max={100} step={0.1} digits={1} unit=":1" onset={(v) => ctl.setCompressor(output, { ratio: v })} />
    <NumberField label="Attack" value={comp.attack} min={0.1} max={2000} step={0.1} digits={1} unit="ms" onset={(v) => ctl.setCompressor(output, { attack: v })} />
    <NumberField label="Release" value={comp.release} min={1} max={5000} step={1} digits={0} unit="ms" onset={(v) => ctl.setCompressor(output, { release: v })} />
  </div>
</section>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 10px 20px;
  }
  .grid :global(.field) {
    display: grid;
    grid-template-columns: 5.5em auto;
    justify-content: start;
  }
</style>
