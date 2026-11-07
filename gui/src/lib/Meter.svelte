<script lang="ts">
  import { untrack } from 'svelte';

  interface Props {
    /** Level in dBFS, or undefined if not available */
    level: number | undefined;
    label?: string;
  }
  let { level, label }: Props = $props();

  const FLOOR = -60;
  const toPercent = (db: number) => Math.max(0, Math.min(100, ((db - FLOOR) / -FLOOR) * 100));

  // Peak hold that slowly falls back
  let peak = $state(FLOOR);
  let peakTime = 0;
  $effect(() => {
    if (level === undefined) return;
    const current = level;
    untrack(() => {
      const now = performance.now();
      if (current >= peak || now - peakTime > 1500) {
        peak = current;
        peakTime = now;
      }
    });
  });

  const fill = $derived(level === undefined ? 0 : toPercent(level));
  const zone = $derived(level === undefined ? '' : level > -3 ? 'hot' : level > -12 ? 'warm' : '');
</script>

<div class="meter" title={level === undefined ? 'No meter' : `${level.toFixed(1)} dBFS`}>
  <div class="track">
    <div class="fill {zone}" style:width="{fill}%"></div>
    {#if level !== undefined}
      <div class="peak" style:left="{toPercent(peak)}%"></div>
    {/if}
  </div>
  <span class="value">
    {#if label}{label}{:else if level === undefined}—{:else}{level <= -120 ? '-∞' : level.toFixed(1)}{/if}
  </span>
</div>

<style>
  .meter {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 3.2rem;
    align-items: center;
    gap: 0.5rem;
  }
  .track {
    position: relative;
    height: 0.55rem;
    border-radius: 999px;
    background: var(--meter-track);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--meter-ok);
    transition: width 120ms linear;
  }
  .fill.warm {
    background: var(--meter-warm);
  }
  .fill.hot {
    background: var(--meter-hot);
  }
  .peak {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--text);
    opacity: 0.6;
  }
  .value {
    font-variant-numeric: tabular-nums;
    font-size: 0.8rem;
    color: var(--text-muted);
    text-align: right;
  }
</style>
