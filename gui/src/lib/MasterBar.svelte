<script lang="ts">
  import { ctl } from './controller.svelte';
  import NumberField from './NumberField.svelte';

  const PRESETS = [0, 1, 2, 3];
  const master = $derived(ctl.master);

  // While dragging, show the local position instead of status updates from the device
  let dragging = $state(false);
  let local = $state(0);
  const shownVolume = $derived(dragging ? local : master.volume);

  // Locks are kept by the daemon, so they protect every browser and the MCP tools too
  const sourceLocked = $derived(ctl.locks.source);
  const presetLocked = $derived(ctl.locks.preset);
</script>

{#snippet lock(what: 'source' | 'preset', locked: boolean, title: string)}
  <label class="lock" class:locked {title}>
    <input type="checkbox" checked={locked} aria-label="Lock {what}" onchange={(e) => ctl.setLock(what, e.currentTarget.checked)} />
    <svg viewBox="0 0 12 14" width="10" height="12" aria-hidden="true">
      <rect x="1" y="6" width="10" height="7" rx="1.2" fill="currentColor" />
      <path d={locked ? 'M3.2 6V4.2a2.8 2.8 0 0 1 5.6 0V6' : 'M3.2 6V4.2a2.8 2.8 0 0 1 5.6 0'} fill="none" stroke="currentColor" stroke-width="1.4" />
    </svg>
    <span>Lock</span>
  </label>
{/snippet}

<div class="master">
  <div class="group volume">
    <label class="label" for="master-volume">Vol</label>
    <input
      id="master-volume"
      type="range"
      min="-127"
      max="0"
      step="0.5"
      value={shownVolume ?? -127}
      disabled={master.volume === undefined}
      oninput={(e) => {
        dragging = true;
        local = Number(e.currentTarget.value);
        ctl.setVolume(local);
      }}
      onchange={(e) => {
        ctl.setVolume(Number(e.currentTarget.value));
        dragging = false;
      }}
      onpointerup={() => (dragging = false)}
    />
    <NumberField
      label="Master volume"
      hideLabel
      value={shownVolume}
      min={-127}
      max={0}
      step={0.5}
      unit="dB"
      width="5.4em"
      onset={(v) => ctl.setVolume(v)}
    />
    <button
      class="danger"
      class:on={master.mute}
      aria-pressed={master.mute ?? false}
      disabled={master.mute === undefined}
      onclick={() => ctl.setMaster({ mute: !master.mute })}
    >
      Mute
    </button>
  </div>

  <div class="group">
    <span class="label" id="preset-label">Preset</span>
    <div class="segmented" role="group" aria-labelledby="preset-label">
      {#each PRESETS as preset (preset)}
        {@const info = preset === master.preset ? ctl.memory.state.slot : ctl.slots[preset]}
        <button
          class:on={master.preset === preset}
          aria-pressed={master.preset === preset}
          title={info?.presetName ? `Slot ${preset + 1}: ${info.presetName}${info.dirty ? ' (modified)' : ''}` : `Slot ${preset + 1}`}
          disabled={presetLocked || ctl.presetBusy || ctl.applyingSlot !== undefined}
          onclick={() => master.preset !== preset && ctl.setMaster({ preset })}
        >
          {preset + 1}
        </button>
      {/each}
    </div>
    <button
      disabled={presetLocked || ctl.previousSlot === undefined || ctl.presetBusy}
      title={presetLocked ? 'Preset is locked' : ctl.previousSlot === undefined ? 'Switch slots once to enable A/B' : `Switch to slot ${ctl.previousSlot + 1}`}
      onclick={() => ctl.toggleAB()}
    >
      A/B
    </button>
    {@render lock('preset', presetLocked, 'Prevent the active preset from being switched by accident')}
  </div>

  {#if ctl.layout && ctl.layout.sources.length > 0}
    <div class="group">
      <label class="label" for="source">Src</label>
      <select
        id="source"
        value={master.source ?? ''}
        disabled={sourceLocked}
        title={sourceLocked ? 'Source is locked. Untick Lock to change it.' : undefined}
        onchange={(e) => ctl.setMaster({ source: e.currentTarget.value })}
      >
        {#if master.source === undefined}<option value="" disabled>—</option>{/if}
        {#each ctl.layout.sources as source (source)}
          <option value={source}>{source.toUpperCase()}</option>
        {/each}
      </select>
      {@render lock('source', sourceLocked, 'Prevent the source from being changed by accident')}
    </div>
  {/if}

  {#if master.dirac !== undefined}
    <div class="group">
      <button class:on={master.dirac} aria-pressed={master.dirac} onclick={() => ctl.setMaster({ dirac: !master.dirac })}>
        Dirac
      </button>
    </div>
  {/if}
</div>

<style>
  .master {
    display: flex;
    align-items: center;
    gap: 6px 20px;
    flex-wrap: wrap;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .volume input[type='range'] {
    width: clamp(90px, 16vw, 200px);
  }
  .segmented {
    display: inline-flex;
  }
  .lock {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: var(--mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    cursor: pointer;
    user-select: none;
  }
  .lock.locked {
    color: var(--accent);
  }
  .lock input {
    margin: 0;
    accent-color: var(--accent);
  }
  select:disabled {
    opacity: 0.75;
    cursor: not-allowed;
  }
  .segmented button {
    border-radius: 0;
    min-width: 28px;
  }
  .segmented button + button {
    margin-left: -1px;
  }
  .segmented button:first-child {
    border-radius: var(--radius) 0 0 var(--radius);
  }
  .segmented button:last-child {
    border-radius: 0 var(--radius) var(--radius) 0;
  }
  .segmented button.on {
    position: relative;
    z-index: 1;
  }
</style>
