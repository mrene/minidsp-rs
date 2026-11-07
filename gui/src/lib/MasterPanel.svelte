<script lang="ts">
  import Fader from './Fader.svelte';
  import type { MasterStatus, Source } from './types';

  interface Props {
    status: MasterStatus;
    sources: Source[];
    presetBusy: boolean;
    onvolume: (db: number) => void;
    onchange: (change: MasterStatus) => void;
  }
  let { status, sources, presetBusy, onvolume, onchange }: Props = $props();

  const PRESETS = [0, 1, 2, 3];
</script>

<section class="card master">
  <header>
    <h2>Master</h2>
    <button
      class="toggle"
      class:on={status.mute}
      aria-pressed={status.mute ?? false}
      disabled={status.mute === undefined}
      onclick={() => onchange({ mute: !status.mute })}
    >
      {status.mute ? 'Muted' : 'Mute'}
    </button>
  </header>

  <Fader
    label="Volume"
    value={status.volume}
    min={-127}
    max={0}
    step={0.5}
    restAt={-127}
    onset={onvolume}
  />

  <div class="row">
    <span class="row-label" id="preset-label">Preset</span>
    <div class="segmented" role="group" aria-labelledby="preset-label">
      {#each PRESETS as preset (preset)}
        <button
          class:active={status.preset === preset}
          aria-pressed={status.preset === preset}
          disabled={presetBusy}
          onclick={() => status.preset !== preset && onchange({ preset })}
        >
          {preset + 1}
        </button>
      {/each}
    </div>
    {#if presetBusy}<span class="hint">Switching…</span>{/if}
  </div>

  {#if sources.length > 0}
    <div class="row">
      <label class="row-label" for="source">Source</label>
      <select
        id="source"
        value={status.source ?? ''}
        onchange={(e) => onchange({ source: e.currentTarget.value })}
      >
        {#if status.source === undefined}<option value="" disabled>—</option>{/if}
        {#each sources as source (source)}
          <option value={source}>{source}</option>
        {/each}
      </select>
    </div>
  {/if}

  {#if status.dirac !== undefined}
    <div class="row">
      <span class="row-label">Dirac Live</span>
      <button
        class="toggle"
        class:on={status.dirac}
        aria-pressed={status.dirac}
        onclick={() => onchange({ dirac: !status.dirac })}
      >
        {status.dirac ? 'On' : 'Off'}
      </button>
    </div>
  {/if}
</section>

<style>
  .master {
    display: grid;
    gap: 0.9rem;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .row-label {
    width: 4.5rem;
    color: var(--text-muted);
    font-size: 0.85rem;
  }
  .hint {
    color: var(--text-muted);
    font-size: 0.8rem;
  }
</style>
