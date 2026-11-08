<script lang="ts">
  import ChannelLabel from './ChannelLabel.svelte';
  import LinkPanel from './LinkPanel.svelte';
  import { channelColor, channelName, ctl, MAX_NAME_LENGTH, type ChannelRef } from './controller.svelte';
  import { peqTypeInfo } from './dsp';
  import CompressorPanel from './CompressorPanel.svelte';
  import CrossoverPanel from './CrossoverPanel.svelte';
  import Fader from './Fader.svelte';
  import FirPanel from './FirPanel.svelte';
  import Meter from './Meter.svelte';
  import NumberField from './NumberField.svelte';
  import PeqTable from './PeqTable.svelte';
  import ResponsePlot, { type Handle, type Trace } from './ResponsePlot.svelte';

  let { ch }: { ch: ChannelRef } = $props();

  const layout = $derived(ch.kind === 'in' ? ctl.layout!.inputs[ch.index] : undefined);
  const outLayout = $derived(ch.kind === 'out' ? ctl.layout!.outputs[ch.index] : undefined);
  const chState = $derived(ctl.state(ch));
  const outState = $derived(ch.kind === 'out' ? ctl.memory.output(ch.index) : undefined);
  const color = $derived(channelColor(ch));
  const peqCount = $derived((layout ?? outLayout)?.peq_count ?? 0);
  const hasGain = $derived(layout ? layout.has_gain : (outLayout?.has_gain ?? false));
  const hasMute = $derived(layout ? layout.has_mute : true);

  let selected = $state<number | undefined>();

  // ---- Renaming ----

  let editing = $state(false);
  let draft = $state('');

  function startRename() {
    draft = ctl.customName(ch) ?? '';
    editing = true;
  }

  function commitRename() {
    if (!editing) return;
    editing = false;
    if (draft.trim() !== (ctl.customName(ch) ?? '')) void ctl.renameChannel(ch, draft);
  }

  function focusAndSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  // ---- Response plot ----

  const fir = $derived(ch.kind === 'out' ? ctl.firResponse(ch.index) : undefined);
  const total = $derived(ctl.totalResponse(ch, fir));
  const knownBands = $derived(Object.keys(chState.peq).length);

  const traces = $derived.by((): Trace[] => {
    const list: Trace[] = [];
    // Individual bands, faint
    for (const [i, band] of Object.entries(chState.peq)) {
      if (band.bypass) continue;
      const values = ctl.bandResponse(band)!;
      const active = Number(i) === selected;
      list.push({ id: `band${i}`, color: active ? color : 'var(--muted)', values, width: active ? 1.25 : 0.75, opacity: active ? 0.9 : 0.45 });
    }
    if (ch.kind === 'out') {
      if (Object.keys(outState!.xover).length) list.push({ id: 'xover', color: 'var(--text)', values: ctl.xoverResponse(ch.index), width: 1, dashed: true, opacity: 0.6 });
      if (fir) list.push({ id: 'fir', color: 'var(--warn)', values: fir, width: 1, dashed: true, opacity: 0.7 });
    }
    list.push({ id: 'total', color, values: total, width: 2 });
    return list;
  });

  const handles = $derived.by((): Handle[] =>
    Object.entries(chState.peq)
      .filter(([, b]) => b.type !== 'RAW' && !b.bypass)
      .map(([i, b]) => ({
        id: Number(i),
        freq: b.freq,
        gain: peqTypeInfo(b.type).hasGain ? b.gain : 0,
        color,
        label: String(Number(i) + 1),
        active: Number(i) === selected,
        movableGain: peqTypeInfo(b.type).hasGain,
      })),
  );

  function dragBand(id: number, freq: number, gain: number) {
    const band = chState.peq[id];
    if (!band) return;
    ctl.setPeq(ch, id, {
      ...band,
      freq: Math.round(freq * (freq < 100 ? 10 : 1)) / (freq < 100 ? 10 : 1),
      gain: peqTypeInfo(band.type).hasGain ? Math.round(gain * 10) / 10 : band.gain,
    });
  }

  function changeQ(id: number, factor: number) {
    const band = chState.peq[id];
    if (!band || band.type === 'RAW') return;
    ctl.setPeq(ch, id, { ...band, q: Math.min(20, Math.max(0.1, Math.round(band.q * factor * 100) / 100)) });
  }

  // Reset the selection when switching channels
  $effect(() => {
    void ch.kind;
    void ch.index;
    selected = undefined;
  });
</script>

<div class="page">
  <header class="panel strip">
    <div class="title">
      <span class="swatch" style:background={color}></span>
      {#if editing}
        <input
          class="rename"
          type="text"
          bind:value={draft}
          maxlength={MAX_NAME_LENGTH}
          placeholder={channelName(ch)}
          aria-label="Name for {channelName(ch)} (empty restores the default)"
          use:focusAndSelect
          onkeydown={(e) => {
            if (e.key === 'Enter') commitRename();
            else if (e.key === 'Escape') editing = false;
          }}
          onblur={commitRename}
        />
      {:else}
        <h1><ChannelLabel {ch} /></h1>
        <button class="ghost" aria-label="Rename {ctl.label(ch)}" onclick={startRename}>Rename</button>
      {/if}
      {#if hasMute}
        <button class="danger" class:on={chState.mute} aria-pressed={chState.mute ?? false} onclick={() => ctl.setMute(ch, !chState.mute)}>
          {chState.mute === undefined ? 'Mute ?' : 'Mute'}
        </button>
      {/if}
      {#if outState}
        <button class:on={outState.invert} aria-pressed={outState.invert ?? false} title="Invert polarity" onclick={() => ctl.setInvert(ch.index, !outState.invert)}>
          Ø {outState.invert === undefined ? '?' : outState.invert ? 'Inv' : 'Norm'}
        </button>
      {/if}
    </div>
    <div class="controls">
      <Meter level={ctl.level(ch)} />
      {#if hasGain}
        <Fader label="Gain" value={chState.gain} min={-72} max={12} onset={(v) => ctl.setGain(ch, v)} />
      {/if}
      {#if outLayout?.has_delay && ctl.layout!.max_delay_ms > 0}
        <div class="delay">
          <NumberField label="Delay" value={outState!.delayMs} min={0} max={Math.floor(ctl.layout!.max_delay_ms * 100) / 100} step={0.01} digits={2} unit="ms" onset={(v) => ctl.setDelay(ch.index, v)} />
          {#if outState!.delayMs !== undefined}
            <span class="dim num">≈ {((outState!.delayMs / 1000) * 343).toFixed(2)} m</span>
          {/if}
        </div>
      {/if}
    </div>
  </header>

  <LinkPanel {ch} />

  <section class="panel">
    <div class="panel-head">
      <h2>Filter response</h2>
      <span class="legend num">
        <span style:color>━ total</span>
        <span class="dim">─ bands</span>
        {#if ch.kind === 'out'}<span class="dim">┄ crossover</span><span style:color="var(--warn)">┄ FIR</span>{/if}
      </span>
    </div>
    <div class="panel-body plot-body">
      <ResponsePlot
        freqs={ctl.freqs}
        {traces}
        {handles}
        yMin={-36}
        yMax={18}
        height={300}
        ondrag={dragBand}
        onq={changeQ}
        onselect={(id) => (selected = id)}
      />
      <p class="hint">
        {#if knownBands < peqCount}
          <span class="unknown">{peqCount - knownBands} of {peqCount} bands unknown, shown flat.</span>
        {/if}
        Drag a handle to move a band · scroll over it to change Q · arrow keys when focused.
      </p>
    </div>
  </section>

  {#if peqCount > 0}
    <PeqTable {ch} count={peqCount} {selected} onselect={(i) => (selected = i)} />
  {/if}

  {#if outLayout}
    <div class="row">
      {#if outLayout.has_crossover && outLayout.crossover_groups > 0}
        <CrossoverPanel output={ch.index} groups={outLayout.crossover_groups} />
      {/if}
      {#if outLayout.has_compressor}
        <CompressorPanel output={ch.index} />
      {/if}
    </div>
    {#if outLayout.has_fir && outLayout.fir_max_taps > 0}
      <FirPanel output={ch.index} maxTaps={outLayout.fir_max_taps} {color} />
    {/if}
  {/if}
</div>

<style>
  .page {
    display: grid;
    /* minmax(0, …) lets wide tables scroll inside their panel instead of widening the page */
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }
  .strip {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 10px 24px;
    align-items: center;
    padding: 12px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .title {
    min-width: 0;
    flex-wrap: wrap;
  }
  .title h1 {
    font-family: var(--mono);
    font-size: 16px;
    letter-spacing: 0.04em;
    min-width: 0;
    max-width: 100%;
    display: inline-flex;
  }
  .rename {
    font-family: var(--sans);
    font-size: 15px;
    width: 16em;
    max-width: 100%;
    padding: 2px 6px;
  }
  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 1px;
  }
  .controls {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 10px 24px;
    align-items: center;
  }
  .delay {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .dim {
    color: var(--dim);
    font-size: 11px;
  }
  .legend {
    display: flex;
    gap: 12px;
    font-size: 10.5px;
  }
  .plot-body {
    padding: 8px 8px 4px;
  }
  .hint {
    margin: 4px 4px 2px;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--dim);
  }
  .row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr));
    gap: 12px;
    align-items: start;
  }
  @media (max-width: 760px) {
    .strip {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
