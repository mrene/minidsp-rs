<script lang="ts">
  import { onMount } from 'svelte';
  import { ctl } from './controller.svelte';

  onMount(() => {
    void ctl.loadPresets();
    ctl.refreshSlots();
  });

  const active = $derived(ctl.master.preset);
  const product = $derived(ctl.layout?.product_name ?? '');
  const busy = $derived(ctl.applyingSlot !== undefined);
  const unknown = $derived(ctl.unknownSettings());
  const activeInfo = $derived(ctl.memory.state.slot);
  const linked = $derived(activeInfo?.presetId ? ctl.presets.find((p) => p.id === activeInfo.presetId) : undefined);

  let name = $state('');
  let notes = $state('');
  let saving = $state(false);
  /** Target slot per library preset */
  let targets = $state<Record<string, number>>({});
  let importInput: HTMLInputElement | undefined = $state();

  async function save() {
    if (!name.trim()) return;
    saving = true;
    await ctl.savePreset(name.trim(), notes.trim());
    saving = false;
    name = '';
    notes = '';
  }

  const fmtDate = (ms: number) =>
    ms ? new Date(ms).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }) : '—';
</script>

<div class="page">
  <section class="panel">
    <div class="panel-head">
      <h2>Device slots</h2>
      <button disabled={ctl.previousSlot === undefined || busy} onclick={() => ctl.toggleAB()} title="Switch between this slot and the previous one">
        A/B {#if ctl.previousSlot !== undefined}⇄ {ctl.previousSlot + 1}{/if}
      </button>
    </div>
    <div class="table-scroll">
      <table class="data">
        <thead><tr><th>Slot</th><th>Holds</th><th>State</th><th><span class="sr-only">Actions</span></th></tr></thead>
        <tbody>
          {#each Array.from({ length: ctl.slotCount }, (_, i) => i) as slot (slot)}
            {@const info = slot === active ? activeInfo : ctl.slots[slot]}
            <tr class:selected={slot === active}>
              <td class="num">{slot + 1}</td>
              <td>{info?.presetName ?? '—'}</td>
              <td>
                <div class="state">
                  {#if ctl.applyingSlot === slot}<span class="busy">Loading…</span>
                  {:else if slot === active}<span class="active">Active</span>{/if}
                  {#if info?.dirty}<span class="dirty" title="Changed since the preset was loaded or saved">Modified</span>{/if}
                </div>
              </td>
              <td class="actions">
                {#if slot !== active}
                  <button disabled={busy || ctl.presetBusy} onclick={() => ctl.setMaster({ preset: slot })}>Activate</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="hint">
      For A/B testing, load each preset into its own slot, then switch slots. Switching is instant;
      loading a preset writes every setting and takes a few seconds (longer with FIR filters).
    </p>
  </section>

  <section class="panel">
    <div class="panel-head"><h2>Save slot {active === undefined ? '' : active + 1} to the library</h2></div>
    <div class="panel-body save">
      <div class="fields">
        <label class="label" for="preset-name">Name</label>
        <input id="preset-name" type="text" bind:value={name} placeholder="e.g. Living room, sub at 80 Hz" maxlength="80" />
        <label class="label" for="preset-notes">Notes</label>
        <input id="preset-notes" type="text" bind:value={notes} placeholder="Optional" maxlength="300" />
      </div>
      <div class="buttons">
        <button class="on" disabled={!name.trim() || saving || busy} onclick={save}>Save as new preset</button>
        {#if linked}
          <button disabled={saving || busy} onclick={() => ctl.updatePreset(linked.id)}>Update “{linked.name}”</button>
        {/if}
      </div>
      {#if unknown.unknown > 0}
        <p class="warning">
          {unknown.unknown} of {unknown.total} settings in this slot are unknown to this app, so the
          preset won't include them. Loading it later leaves those settings as they are in the target slot.
        </p>
      {/if}
    </div>
  </section>

  <section class="panel">
    <div class="panel-head">
      <h2>Library · {ctl.presets.length} presets</h2>
      <div>
        <input bind:this={importInput} type="file" accept=".json,application/json" class="sr-only" tabindex="-1"
          onchange={(e) => {
            const file = e.currentTarget.files?.[0];
            e.currentTarget.value = '';
            if (file) void ctl.importPreset(file);
          }} />
        <button onclick={() => importInput?.click()}>Import file…</button>
      </div>
    </div>
    {#if !ctl.presetsLoaded}
      <p class="empty">Loading…</p>
    {:else if ctl.presets.length === 0}
      <p class="empty">No presets yet. Set up a slot, then save it above.</p>
    {:else}
      <div class="table-scroll">
        <table class="data">
          <thead><tr><th>Name</th><th>Product</th><th>Updated</th><th>Load into</th><th><span class="sr-only">Actions</span></th></tr></thead>
          <tbody>
            {#each ctl.presets as preset (preset.id)}
              {@const compatible = preset.product_name === product}
              {@const target = targets[preset.id] ?? active ?? 0}
              <tr>
                <td>
                  <div class="name">{preset.name}</div>
                  {#if preset.notes}<div class="notes">{preset.notes}</div>{/if}
                </td>
                <td class:mismatch={!compatible} title={compatible ? '' : `Made for a ${preset.product_name}`}>{preset.product_name}</td>
                <td class="date">{fmtDate(preset.updated_ms)}</td>
                <td>
                  <div class="load">
                  <select aria-label="Slot to load “{preset.name}” into" value={target} disabled={!compatible}
                    onchange={(e) => (targets[preset.id] = Number(e.currentTarget.value))}>
                    {#each Array.from({ length: ctl.slotCount }, (_, i) => i) as slot (slot)}
                      <option value={slot}>Slot {slot + 1}</option>
                    {/each}
                  </select>
                  <button class="on" disabled={!compatible || busy} onclick={() => ctl.applyPreset(preset.id, target)}>Load</button>
                  </div>
                </td>
                <td class="actions">
                  <button class="ghost" onclick={() => ctl.exportPreset(preset.id)}>Export</button>
                  <button class="ghost danger"
                    onclick={() => confirm(`Delete preset “${preset.name}”? This removes its file.`) && ctl.deletePreset(preset.id)}>
                    Delete
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </section>
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }
  .state {
    display: flex;
    gap: 8px;
  }
  .active {
    color: var(--accent);
  }
  .busy {
    color: var(--warn);
  }
  .dirty {
    color: var(--warn);
    font-size: 10.5px;
    border: 1px solid currentColor;
    border-radius: 2px;
    padding: 0 4px;
  }
  .actions {
    text-align: right;
  }
  .actions button + button {
    margin-left: 4px;
  }
  .hint,
  .empty {
    margin: 0;
    padding: 10px 12px;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--dim);
  }
  .save {
    display: grid;
    gap: 10px;
  }
  .fields {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 8px 12px;
    align-items: center;
    max-width: 640px;
  }
  .fields input {
    width: 100%;
  }
  .buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .warning {
    margin: 0;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--warn);
    max-width: 720px;
  }
  .name {
    font-family: var(--sans);
    font-size: 13px;
  }
  .notes {
    color: var(--muted);
    font-family: var(--sans);
    font-size: 11.5px;
    white-space: normal;
  }
  .date {
    color: var(--muted);
  }
  .mismatch {
    color: var(--danger);
  }
  .load {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  button.danger:hover:not(:disabled) {
    color: var(--danger);
  }
</style>
