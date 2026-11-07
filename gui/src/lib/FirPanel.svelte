<script lang="ts">
  import { ctl } from './controller.svelte';
  import { firDb, parseTextTaps, parseWav } from './dsp';
  import ResponsePlot from './ResponsePlot.svelte';
  import { notify, reportError } from './toasts.svelte';

  interface Props {
    output: number;
    maxTaps: number;
    color: string;
  }
  let { output, maxTaps, color }: Props = $props();

  const fir = $derived(ctl.memory.output(output).fir);
  const loading = $derived(ctl.firLoading[output] ?? false);

  /** A file that was read but not loaded to the device yet */
  let pending = $state.raw<{ name: string; taps: number[] } | undefined>();
  let fileInput: HTMLInputElement | undefined = $state();

  const loadedResponse = $derived(ctl.firResponse(output));
  const pendingResponse = $derived(pending ? firDb(pending.taps, ctl.freqs, ctl.fs) : undefined);

  async function pick(event: Event & { currentTarget: HTMLInputElement }) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (!file) return;
    try {
      const parsed = file.name.toLowerCase().endsWith('.wav')
        ? parseWav(await file.arrayBuffer())
        : parseTextTaps(await file.text());
      if (parsed.sampleRate && parsed.sampleRate !== ctl.fs) {
        throw new Error(`${file.name} is ${parsed.sampleRate} Hz, but this device's filters run at ${ctl.fs} Hz`);
      }
      if (parsed.taps.length > maxTaps) {
        throw new Error(`${file.name} has ${parsed.taps.length} taps; this output accepts at most ${maxTaps}`);
      }
      pending = { name: file.name, taps: parsed.taps };
    } catch (e) {
      reportError(e);
    }
  }

  async function load() {
    if (!pending) return;
    const { name, taps } = pending;
    if (await ctl.loadFir(output, taps, name)) notify(`Loaded ${taps.length} taps into OUT ${output + 1}`);
    pending = undefined;
  }

  const traces = $derived([
    ...(loadedResponse ? [{ id: 'loaded', color, values: loadedResponse, width: 1.5 }] : []),
    ...(pendingResponse ? [{ id: 'pending', color: 'var(--warn)', values: pendingResponse, width: 1.25, dashed: true }] : []),
  ]);
</script>

<section class="panel">
  <div class="panel-head">
    <h2>FIR · {maxTaps} taps max</h2>
    <button class="warn" class:on={fir.bypass} aria-pressed={fir.bypass ?? false} onclick={() => ctl.setFirBypass(output, !fir.bypass)}>
      {fir.bypass === undefined ? 'Bypass ?' : 'Bypass'}
    </button>
  </div>
  <div class="panel-body">
    <div class="status num">
      {#if loading}
        <span>Loading {pending?.taps.length ?? ''} taps…</span>
      {:else if fir.taps}
        <span>{fir.name}</span><span class="dim">{fir.taps.length} / {maxTaps} taps</span>
      {:else}
        <span class="unknown">Unknown. The device can't report its FIR filter.</span>
      {/if}
    </div>

    {#if traces.length}
      <ResponsePlot freqs={ctl.freqs} {traces} height={160} yMin={-24} yMax={12} />
    {/if}

    {#if pending && !loading}
      <div class="pending">
        <span class="num">{pending.name} · {pending.taps.length} taps <span class="dim">(dashed)</span></span>
        <button class="on" onclick={load}>Load to device</button>
        <button class="ghost" onclick={() => (pending = undefined)}>Cancel</button>
      </div>
    {/if}

    <div class="actions">
      <input bind:this={fileInput} type="file" accept=".wav,.txt,.csv,audio/wav,text/plain" class="sr-only" tabindex="-1" onchange={pick} />
      <button disabled={loading} onclick={() => fileInput?.click()}>Open WAV / text…</button>
      <button disabled={loading} onclick={() => ctl.resetFir(output)} title="Load a single unit impulse: a flat, transparent filter">Reset to flat</button>
    </div>
  </div>
</section>

<style>
  .panel-body {
    display: grid;
    gap: 10px;
  }
  .status {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 12px;
  }
  .dim {
    color: var(--dim);
  }
  .pending,
  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .pending .num {
    font-size: 12px;
    color: var(--warn);
    margin-right: auto;
  }
</style>
