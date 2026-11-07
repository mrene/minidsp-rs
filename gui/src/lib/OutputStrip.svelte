<script lang="ts">
  import Fader from './Fader.svelte';
  import Meter from './Meter.svelte';
  import type { OutputState } from './channels.svelte';
  import type { OutputLayout } from './types';

  interface Props {
    index: number;
    layout: OutputLayout;
    state: OutputState;
    level: number | undefined;
    maxDelayMs: number;
    ongain: (db: number) => void;
    onmute: (mute: boolean) => void;
    oninvert: (invert: boolean) => void;
    ondelay: (ms: number) => void;
  }
  let { index, layout, state, level, maxDelayMs, ongain, onmute, oninvert, ondelay }: Props = $props();
</script>

<div class="strip">
  <div class="strip-head">
    <h3>Output {index + 1}</h3>
    <div class="buttons">
      <button class="toggle small" class:on={state.invert} aria-pressed={state.invert ?? false} title="Invert polarity" onclick={() => oninvert(!state.invert)}>
        Ø
      </button>
      <button class="toggle small" class:on={state.mute} aria-pressed={state.mute ?? false} onclick={() => onmute(!state.mute)}>
        {state.mute ? 'Muted' : 'Mute'}
      </button>
    </div>
  </div>
  {#if layout.has_meter}<Meter {level} />{/if}
  {#if layout.has_gain}
    <Fader label="Gain" value={state.gain} min={-72} max={12} onset={ongain} />
  {/if}
  {#if layout.has_delay && maxDelayMs > 0}
    <Fader
      label="Delay"
      value={state.delayMs}
      min={0}
      max={Math.floor(maxDelayMs * 100) / 100}
      step={0.01}
      unit="ms"
      digits={2}
      onset={ondelay}
    />
  {/if}
</div>

<style>
  .buttons {
    display: flex;
    gap: 0.4rem;
  }
</style>
