<script lang="ts">
  import { ctl } from './controller.svelte';
  import NumberField from './NumberField.svelte';

  const PRESETS = [0, 1, 2, 3];
  const master = $derived(ctl.master);

  // While dragging, show the local position instead of status updates from the device
  let dragging = $state(false);
  let local = $state(0);
  const shownVolume = $derived(dragging ? local : master.volume);
</script>

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
        <button
          class:on={master.preset === preset}
          aria-pressed={master.preset === preset}
          disabled={ctl.presetBusy}
          onclick={() => master.preset !== preset && ctl.setMaster({ preset })}
        >
          {preset + 1}
        </button>
      {/each}
    </div>
  </div>

  {#if ctl.layout && ctl.layout.sources.length > 0}
    <div class="group">
      <label class="label" for="source">Src</label>
      <select id="source" value={master.source ?? ''} onchange={(e) => ctl.setMaster({ source: e.currentTarget.value })}>
        {#if master.source === undefined}<option value="" disabled>—</option>{/if}
        {#each ctl.layout.sources as source (source)}
          <option value={source}>{source.toUpperCase()}</option>
        {/each}
      </select>
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
