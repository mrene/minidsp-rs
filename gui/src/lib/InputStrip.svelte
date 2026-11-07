<script lang="ts">
  import Fader from './Fader.svelte';
  import Meter from './Meter.svelte';
  import type { InputState } from './channels.svelte';
  import type { InputLayout } from './types';

  interface Props {
    index: number;
    layout: InputLayout;
    state: InputState;
    level: number | undefined;
    ongain: (db: number) => void;
    onmute: (mute: boolean) => void;
  }
  let { index, layout, state, level, ongain, onmute }: Props = $props();
</script>

<div class="strip">
  <div class="strip-head">
    <h3>Input {index + 1}</h3>
    {#if layout.has_mute}
      <button class="toggle small" class:on={state.mute} aria-pressed={state.mute ?? false} onclick={() => onmute(!state.mute)}>
        {state.mute ? 'Muted' : 'Mute'}
      </button>
    {/if}
  </div>
  {#if layout.has_meter}<Meter {level} />{/if}
  {#if layout.has_gain}
    <Fader label="Gain" value={state.gain} min={-72} max={12} onset={ongain} />
  {/if}
</div>
