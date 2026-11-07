<script lang="ts">
  import { onMount } from 'svelte';
  import { ctl } from './controller.svelte';
  import type { HistoryEntry } from './types';

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
  const slotNumbers = $derived(Array.from({ length: ctl.slotCount }, (_, i) => i));

  // ---- Saving the active slot ----

  let newName = $state('');
  let newNotes = $state('');
  let saving = $state(false);

  async function save() {
    if (!newName.trim()) return;
    saving = true;
    const id = await ctl.savePreset(newName.trim(), newNotes.trim());
    saving = false;
    if (id) {
      newName = '';
      newNotes = '';
      selectedId = id;
    }
  }

  // ---- Selected preset ----

  let selectedId = $state<string | undefined>();
  const selected = $derived(ctl.presets.find((p) => p.id === selectedId));
  let history = $state<HistoryEntry[]>([]);
  let historyFor = $state('');
  let target = $state(0);
  let renaming = $state(false);
  let draftName = $state('');
  let draftNotes = $state('');

  // Keep a valid selection and load its history whenever it (or its version) changes
  $effect(() => {
    if (!ctl.presetsLoaded) return;
    if (!selected && ctl.presets.length) selectedId = ctl.presets[0].id;
  });
  $effect(() => {
    const p = selected;
    if (!p) return;
    const key = `${p.id}@${p.version}`;
    if (key === historyFor) return;
    historyFor = key;
    draftNotes = p.notes;
    void ctl.presetHistory(p.id).then((h) => {
      if (historyFor === key) history = h;
    });
  });
  $effect(() => {
    // Default the load target to the active slot when the selection changes
    void selectedId;
    target = active ?? 0;
  });

  function startRename() {
    draftName = selected?.name ?? '';
    renaming = true;
  }

  async function commitRename() {
    if (!renaming || !selected) return;
    renaming = false;
    const name = draftName.trim();
    if (!name || name === selected.name) return;
    const id = await ctl.renamePreset(selected.id, name);
    if (id) selectedId = id;
  }

  function focusAndSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  const fmt = (ms: number) =>
    ms ? new Date(ms).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'medium' }) : '—';
  const fmtShort = (ms: number) =>
    ms ? new Date(ms).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' }) : '—';

  /** The latest change can be undone unless it's the first version */
  const canUndo = $derived.by(() => {
    const last = history[0];
    if (!last) return false;
    return (last.restored_from ?? last.version) > 1;
  });

  let importInput: HTMLInputElement | undefined = $state();
</script>

<div class="page">
  <!-- Device slots -->
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
          {#each slotNumbers as slot (slot)}
            {@const info = slot === active ? activeInfo : ctl.slots[slot]}
            <tr class:selected={slot === active}>
              <td class="num">{slot + 1}</td>
              <td>
                {#if info?.presetId && ctl.presets.some((p) => p.id === info.presetId)}
                  <button class="link" onclick={() => (selectedId = info.presetId)}>{info.presetName}</button>
                {:else}{info?.presetName ?? '—'}{/if}
              </td>
              <td>
                <div class="state">
                  {#if ctl.applyingSlot === slot}<span class="busy">Loading…</span>
                  {:else if slot === active}<span class="active">Active</span>{/if}
                  {#if info?.dirty}<span class="badge" title="The slot and its preset differ: one of them changed since loading or saving">Modified</span>{/if}
                </div>
              </td>
              <td class="right">
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

  <!-- Save the active slot -->
  <section class="panel">
    <div class="panel-head"><h2>Save slot {active === undefined ? '' : active + 1} to the library</h2></div>
    <div class="panel-body save">
      <div class="fields">
        <label class="label" for="preset-name">Name</label>
        <input id="preset-name" type="text" bind:value={newName} placeholder="e.g. Living room, sub at 80 Hz" maxlength="80"
          onkeydown={(e) => e.key === 'Enter' && save()} />
        <label class="label" for="preset-notes">Notes</label>
        <input id="preset-notes" type="text" bind:value={newNotes} placeholder="Optional" maxlength="300" />
      </div>
      <div class="buttons">
        <button class="on" disabled={!newName.trim() || saving || busy} onclick={save}>Save as new preset</button>
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

  <!-- Library and selected preset -->
  <div class="manager">
    <section class="panel library">
      <div class="panel-head">
        <h2>Library · {ctl.presets.length}</h2>
        <input bind:this={importInput} type="file" accept=".json,application/json" class="sr-only" tabindex="-1"
          onchange={(e) => {
            const file = e.currentTarget.files?.[0];
            e.currentTarget.value = '';
            if (file) void ctl.importPreset(file);
          }} />
        <button onclick={() => importInput?.click()}>Import…</button>
      </div>
      {#if !ctl.presetsLoaded}
        <p class="hint">Loading…</p>
      {:else if ctl.presets.length === 0}
        <p class="hint">No presets yet. Set up a slot, then save it above.</p>
      {:else}
        <ul class="list" aria-label="Presets">
          {#each ctl.presets as preset (preset.id)}
            <li>
              <button class="item" class:current={preset.id === selectedId} aria-current={preset.id === selectedId ? 'true' : undefined}
                onclick={() => (selectedId = preset.id)}>
                <span class="item-name">{preset.name}</span>
                <span class="item-meta num">
                  {#if preset.product_name !== product}<span class="mismatch">{preset.product_name}</span> · {/if}v{preset.version} · {fmtShort(preset.updated_ms)}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section class="panel detail">
      {#if selected}
        {@const compatible = selected.product_name === product}
        <div class="panel-head">
          {#if renaming}
            <input class="rename" type="text" bind:value={draftName} maxlength="80" aria-label="New name for “{selected.name}”"
              use:focusAndSelect
              onkeydown={(e) => {
                if (e.key === 'Enter') commitRename();
                else if (e.key === 'Escape') renaming = false;
              }}
              onblur={commitRename} />
          {:else}
            <h1 class="title">{selected.name}</h1>
            <button class="ghost" onclick={startRename}>Rename</button>
          {/if}
        </div>
        <div class="panel-body detail-body">
          <dl class="facts num">
            <dt>Product</dt><dd class:mismatch={!compatible}>{selected.product_name}{compatible ? '' : ' (not this device)'}</dd>
            <dt>Version</dt><dd>{selected.version}</dd>
            <dt>Updated</dt><dd>{fmt(selected.updated_ms)}</dd>
            <dt>File</dt><dd>{selected.id}.json</dd>
          </dl>

          <div class="notes">
            <label class="label" for="preset-detail-notes">Notes</label>
            <textarea id="preset-detail-notes" rows="2" maxlength="300" bind:value={draftNotes}></textarea>
            {#if draftNotes !== selected.notes}
              <div class="buttons">
                <button class="on" onclick={() => ctl.setPresetNotes(selected.id, draftNotes)}>Save notes</button>
                <button class="ghost" onclick={() => (draftNotes = selected.notes)}>Discard</button>
              </div>
            {/if}
          </div>

          <div class="actions">
            <div class="load">
              <select aria-label="Slot to load “{selected.name}” into" bind:value={target} disabled={!compatible}>
                {#each slotNumbers as slot (slot)}<option value={slot}>Slot {slot + 1}</option>{/each}
              </select>
              <button class="on" disabled={!compatible || busy} onclick={() => ctl.applyPreset(selected.id, target)}>Load into slot</button>
            </div>
            <button disabled={!canUndo} onclick={() => ctl.undoPreset(selected.id)} title="Restore the settings before the last change">Undo last change</button>
            <button onclick={() => ctl.exportPreset(selected.id)}>Export</button>
            <button class="danger-text"
              onclick={() => confirm(`Delete “${selected.name}”? It can be restored later from Deleted presets.`) && ctl.deletePreset(selected.id)}>
              Delete
            </button>
          </div>

          <div class="history">
            <h2>History</h2>
            <div class="table-scroll">
              <table class="data">
                <thead><tr><th>Ver</th><th>Date and time</th><th>Change</th><th><span class="sr-only">Roll back</span></th></tr></thead>
                <tbody>
                  {#each history as entry, i (entry.version)}
                    <tr>
                      <td class="num">{entry.version}</td>
                      <td class="num when">{fmt(entry.at_ms)}</td>
                      <td class="change">{entry.action}</td>
                      <td class="right">
                        {#if i === 0}
                          <span class="badge current-badge">Current</span>
                        {:else if !entry.deleted}
                          <button onclick={() => confirm(`Roll “${selected.name}” back to version ${entry.version}? This is recorded as a new version, so it can be undone.`) && ctl.rollbackPreset(selected.id, entry.version)}>
                            Roll back
                          </button>
                        {/if}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
            <p class="hint">
              Undo and roll back restore settings and notes, and are recorded as new versions, so they
              can be undone too. They keep the current name; to undo a rename, rename it back.
            </p>
          </div>
        </div>
      {:else}
        <p class="hint">Select a preset to see its details and history.</p>
      {/if}
    </section>
  </div>

  {#if ctl.deletedPresets.length}
    <section class="panel">
      <div class="panel-head"><h2>Deleted presets · {ctl.deletedPresets.length}</h2></div>
      <div class="table-scroll">
        <table class="data">
          <thead><tr><th>Name</th><th>Deleted</th><th><span class="sr-only">Restore</span></th></tr></thead>
          <tbody>
            {#each ctl.deletedPresets as preset (preset.id)}
              <tr>
                <td>{preset.name}</td>
                <td class="num">{fmt(preset.updated_ms)}</td>
                <td class="right"><button onclick={() => ctl.restorePreset(preset.id)}>Restore</button></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }
  .manager {
    display: grid;
    grid-template-columns: minmax(220px, 300px) minmax(0, 1fr);
    gap: 12px;
    align-items: start;
  }
  @media (max-width: 900px) {
    .manager {
      grid-template-columns: minmax(0, 1fr);
    }
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
  .badge {
    color: var(--warn);
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    border: 1px solid currentColor;
    border-radius: 2px;
    padding: 0 4px;
  }
  .current-badge {
    color: var(--accent);
  }
  .right {
    text-align: right;
  }
  .hint {
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

  /* Library list */
  .list {
    list-style: none;
    margin: 0;
    padding: 4px;
    max-height: 560px;
    overflow-y: auto;
  }
  .item {
    display: grid;
    gap: 2px;
    width: 100%;
    text-align: left;
    text-transform: none;
    letter-spacing: 0;
    font-family: var(--sans);
    font-size: 13px;
    background: none;
    border: 1px solid transparent;
    border-left: 2px solid transparent;
    padding: 7px 10px;
  }
  .item:hover {
    background: var(--panel-2);
  }
  .item.current {
    background: var(--raised);
    border-color: var(--line);
    border-left-color: var(--accent);
  }
  .item-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item-meta {
    font-size: 10.5px;
    color: var(--muted);
  }
  .mismatch {
    color: var(--danger);
  }

  /* Selected preset */
  .title {
    font-size: 15px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    font-family: var(--sans);
    font-size: 14px;
    width: 100%;
    max-width: 28em;
  }
  .detail-body {
    display: grid;
    gap: 14px;
  }
  .facts {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 4px 14px;
    margin: 0;
    font-size: 12px;
  }
  .facts dt {
    color: var(--muted);
    font-size: 10.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .facts dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .notes {
    display: grid;
    gap: 6px;
  }
  textarea {
    font: inherit;
    font-size: 13px;
    color: inherit;
    background: var(--panel-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    padding: 6px 8px;
    resize: vertical;
    width: 100%;
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
  }
  .load {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-right: auto;
  }
  .danger-text:hover:not(:disabled) {
    color: var(--danger);
    border-color: var(--danger);
  }
  .history {
    display: grid;
    gap: 6px;
  }
  .history .hint {
    padding: 4px 0 0;
  }
  .when {
    color: var(--muted);
  }
  .change {
    white-space: normal;
    font-family: var(--sans);
  }
  button.link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    font-family: var(--sans);
    font-size: 13px;
    text-transform: none;
    letter-spacing: 0;
  }
</style>
