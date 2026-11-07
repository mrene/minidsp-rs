<script lang="ts">
  import type { PeqBand } from './channels.svelte';
  import { channelName, ctl, type ChannelRef } from './controller.svelte';
  import { parseRewBiquads, PEQ_TYPES, peqTypeInfo, type PeqType } from './dsp';
  import NumberField from './NumberField.svelte';
  import { notify, reportError } from './toasts.svelte';

  interface Props {
    ch: ChannelRef;
    count: number;
    selected: number | undefined;
    onselect: (index: number) => void;
  }
  let { ch, count, selected, onselect }: Props = $props();

  const bands = $derived(ctl.state(ch).peq);
  const indexes = $derived(Array.from({ length: count }, (_, i) => i));

  /** Defaults for a band that hasn't been set yet, spread across the spectrum */
  function defaultBand(index: number, type: PeqType): PeqBand {
    const freq = Math.round(30 * Math.pow(16000 / 30, index / Math.max(1, count - 1)));
    return { type, freq, gain: 0, q: type === 'PK' ? 1 : 0.707, bypass: false };
  }

  function update(index: number, change: Partial<PeqBand>) {
    const current = bands[index] ?? defaultBand(index, 'PK');
    ctl.setPeq(ch, index, { ...current, ...change });
  }

  function setType(index: number, type: PeqType) {
    const current = bands[index];
    if (!current) ctl.setPeq(ch, index, defaultBand(index, type));
    else update(index, { type, raw: undefined });
  }

  async function flattenAll() {
    const all: Record<number, PeqBand> = {};
    for (const i of indexes) all[i] = { ...defaultBand(i, 'PK'), gain: 0 };
    await ctl.setPeqs(ch, all);
    notify(`${channelName(ch)}: all ${count} bands set flat`);
  }

  async function setAllBypass(bypass: boolean) {
    const all: Record<number, PeqBand> = {};
    for (const i of indexes) if (bands[i]) all[i] = { ...bands[i], bypass };
    if (Object.keys(all).length) await ctl.setPeqs(ch, all);
  }

  let fileInput: HTMLInputElement | undefined = $state();

  async function importRew(event: Event & { currentTarget: HTMLInputElement }) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (!file) return;
    try {
      const filters = parseRewBiquads(await file.text());
      if (filters.length === 0) throw new Error(`No miniDSP biquads found in ${file.name}`);

      // REW numbers its filters from 1. Bands not in the file are set flat so nothing is left over.
      const all: Record<number, PeqBand> = {};
      for (const i of indexes) all[i] = { ...defaultBand(i, 'PK'), gain: 0 };
      let skipped = 0;
      for (const f of filters) {
        const i = f.index - 1;
        if (i >= 0 && i < count) all[i] = { type: 'RAW', freq: 0, gain: 0, q: 0, bypass: false, raw: f.coeffs };
        else skipped++;
      }
      await ctl.setPeqs(ch, all);
      notify(
        `Imported ${filters.length - skipped} filters into ${channelName(ch)}` +
          (skipped ? ` (${skipped} skipped: this channel has ${count} bands)` : ''),
      );
    } catch (e) {
      reportError(e);
    }
  }

  const anyKnown = $derived(Object.keys(bands).length > 0);
  const allBypassed = $derived(anyKnown && Object.values(bands).every((b) => b.bypass));
</script>

<section class="panel">
  <div class="panel-head">
    <h2>Parametric EQ · {count} bands</h2>
    <div class="tools">
      <input bind:this={fileInput} type="file" accept=".txt,text/plain" class="sr-only" tabindex="-1" onchange={importRew} />
      <button onclick={() => fileInput?.click()} title="Import filters exported from REW as miniDSP biquads">Import REW</button>
      <button class="warn" class:on={allBypassed} disabled={!anyKnown} onclick={() => setAllBypass(!allBypassed)}>Bypass all</button>
      <button onclick={flattenAll}>Flatten</button>
    </div>
  </div>
  <div class="table-scroll">
    <table class="data">
      <thead>
        <tr>
          <th>#</th>
          <th>Type</th>
          <th>Freq</th>
          <th>Gain</th>
          <th>Q</th>
          <th><span class="sr-only">Bypass</span></th>
        </tr>
      </thead>
      <tbody>
        {#each indexes as i (i)}
          {@const band = bands[i]}
          {@const info = band ? peqTypeInfo(band.type) : undefined}
          <tr class:selected={selected === i} class:bypassed={band?.bypass} onfocusin={() => onselect(i)} onclick={() => onselect(i)}>
            <td class="idx">{i + 1}</td>
            <td>
              <select
                aria-label="Band {i + 1} type"
                value={band?.type ?? ''}
                onchange={(e) => setType(i, e.currentTarget.value as PeqType)}
              >
                {#if !band}<option value="" disabled>— unknown</option>{/if}
                {#each PEQ_TYPES as t (t.id)}
                  {#if t.id !== 'RAW' || band?.type === 'RAW'}
                    <option value={t.id}>{t.id} · {t.label}</option>
                  {/if}
                {/each}
              </select>
            </td>
            {#if band?.type === 'RAW'}
              <td colspan="3" class="raw">
                b0 {band.raw?.b0.toFixed(5)} · a1 {band.raw?.a1.toFixed(5)} · a2 {band.raw?.a2.toFixed(5)}
              </td>
            {:else}
              <td>
                <NumberField label="Band {i + 1} frequency" hideLabel value={band?.freq} min={10} max={ctl.fs * 0.49} step={1} digits={0} unit="Hz" width="5.6em" disabled={!band} onset={(v) => update(i, { freq: v })} />
              </td>
              <td>
                {#if !band || info?.hasGain}
                  <NumberField label="Band {i + 1} gain" hideLabel value={band?.gain} min={-24} max={24} step={0.1} digits={1} unit="dB" width="4.6em" disabled={!band} onset={(v) => update(i, { gain: v })} />
                {:else}<span class="unknown">—</span>{/if}
              </td>
              <td>
                <NumberField label="Band {i + 1} Q" hideLabel value={band?.q} min={0.1} max={20} step={0.01} digits={2} width="4.4em" disabled={!band} onset={(v) => update(i, { q: v })} />
              </td>
            {/if}
            <td>
              <button class="warn" class:on={band?.bypass} aria-pressed={band?.bypass ?? false} disabled={!band} onclick={() => update(i, { bypass: !band?.bypass })}>
                Byp
              </button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</section>

<style>
  .tools {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .idx {
    color: var(--muted);
    width: 2em;
  }
  .raw {
    color: var(--muted);
    font-size: 11px;
  }
  tr {
    cursor: default;
  }
</style>
