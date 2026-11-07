<script lang="ts">
  import { untrack } from 'svelte';

  interface Props {
    /** Level in dBFS, or undefined if not available */
    level: number | undefined;
    /** Show the numeric value next to the bar */
    showValue?: boolean;
  }
  let { level, showValue = true }: Props = $props();

  const FLOOR = -72;
  const toPercent = (db: number) => Math.max(0, Math.min(100, ((db - FLOOR) / -FLOOR) * 100));

  // Peak hold that falls back after a moment
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
  const TICKS = [-60, -48, -36, -24, -12, -6];
</script>

<div class="meter" class:compact={!showValue} title={level === undefined ? 'No meter' : `${level.toFixed(1)} dBFS`}>
  <div class="track">
    <div class="fill" style:clip-path="inset(0 {100 - fill}% 0 0)"></div>
    {#each TICKS as t (t)}<span class="tick" style:left="{toPercent(t)}%"></span>{/each}
    {#if level !== undefined}
      <div class="peak" style:left="{toPercent(peak)}%"></div>
    {/if}
  </div>
  {#if showValue}
    <span class="value num">
      {#if level === undefined}—{:else if level <= -120}−∞{:else}{level.toFixed(1)}{/if}
    </span>
  {/if}
</div>

<style>
  .meter {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 3.4rem;
    align-items: center;
    gap: 8px;
  }
  .meter.compact {
    grid-template-columns: minmax(0, 1fr);
  }
  .track {
    position: relative;
    height: 6px;
    background: var(--grid);
    border: 1px solid var(--line);
    border-radius: 1px;
  }
  .fill {
    position: absolute;
    inset: 0;
    /* Green up to -12, amber to -3, red above; revealed by the clip path */
    background: linear-gradient(
      90deg,
      var(--ok) 0%,
      var(--ok) 83.3%,
      var(--warn) 83.3%,
      var(--warn) 95.8%,
      var(--danger) 95.8%
    );
    transition: clip-path 120ms linear;
  }
  .tick {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--bg);
    opacity: 0.6;
  }
  .peak {
    position: absolute;
    top: -1px;
    bottom: -1px;
    width: 2px;
    background: var(--text);
  }
  .value {
    font-size: 11.5px;
    color: var(--muted);
    text-align: right;
  }
</style>
