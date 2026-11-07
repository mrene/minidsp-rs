<script lang="ts">
  import ChannelLabel from './ChannelLabel.svelte';
  import { channelColor, channelName, ctl, type ChannelRef } from './controller.svelte';
  import Meter from './Meter.svelte';
  import NumberField from './NumberField.svelte';
  import ResponsePlot, { type Trace } from './ResponsePlot.svelte';
  import { href } from './route.svelte';
  import { slope } from './dsp';

  const outputs = $derived<ChannelRef[]>((ctl.layout?.outputs ?? []).map((_, index) => ({ kind: 'out', index })));
  const inputs = $derived<ChannelRef[]>((ctl.layout?.inputs ?? []).map((_, index) => ({ kind: 'in', index })));

  let hidden = $state<Record<number, boolean>>({});

  const traces = $derived(
    outputs
      .filter((ch) => !hidden[ch.index])
      .map((ch): Trace => ({ id: `out${ch.index}`, color: channelColor(ch), values: ctl.totalResponse(ch, ctl.firResponse(ch.index)), width: 1.5 })),
  );

  function summary(ch: ChannelRef): string {
    const s = ctl.state(ch);
    const parts: string[] = [];
    const group = ctl.linkGroup(ch);
    if (group) parts.push(`⇄ ${group.channels.filter((i) => i !== ch.index).map((i) => ctl.label({ kind: ch.kind, index: i })).join(' + ')}`);
    const active = Object.values(s.peq).filter((b) => !b.bypass).length;
    if (active) parts.push(`PEQ ${active}`);
    if (ch.kind === 'out') {
      const o = ctl.memory.output(ch.index);
      for (const g of Object.values(o.xover)) {
        if (!g.bypass) parts.push(`${g.type} ${g.kind}${slope(g.order)} ${Math.round(g.freq)}`);
      }
      if (o.fir.taps && !o.fir.bypass) parts.push(`FIR ${o.fir.taps.length}`);
      if (o.compressor.bypass === false) parts.push('COMP');
    }
    return parts.join(' · ');
  }
</script>

<div class="page">
  <section class="panel">
    <div class="panel-head">
      <h2>Output response</h2>
      <div class="legend">
        {#each outputs as ch (ch.index)}
          <button class="ghost chip" class:off={hidden[ch.index]} aria-pressed={!hidden[ch.index]} onclick={() => (hidden[ch.index] = !hidden[ch.index])}>
            <span class="swatch" style:background={channelColor(ch)}></span><ChannelLabel {ch} showTag={false} />
          </button>
        {/each}
      </div>
    </div>
    <div class="panel-body plot-body">
      <ResponsePlot freqs={ctl.freqs} {traces} yMin={-36} yMax={18} height={280} />
    </div>
  </section>

  <section class="panel">
    <div class="panel-head"><h2>Channels</h2></div>
    <div class="table-scroll">
      <table class="data">
        <thead>
          <tr>
            <th>Channel</th>
            <th class="meter-col">Level</th>
            <th>Gain</th>
            <th><span class="sr-only">Mute</span></th>
            <th>Pol</th>
            <th>Delay</th>
            <th>Filters</th>
          </tr>
        </thead>
        <tbody>
          {#each [...inputs, ...outputs] as ch (`${ch.kind}${ch.index}`)}
            {@const s = ctl.state(ch)}
            {@const out = ch.kind === 'out' ? ctl.memory.output(ch.index) : undefined}
            {@const outLayout = ch.kind === 'out' ? ctl.layout!.outputs[ch.index] : undefined}
            {@const inLayout = ch.kind === 'in' ? ctl.layout!.inputs[ch.index] : undefined}
            <tr>
              <td>
                <a class="ch" href={href.channel(ch)}><span class="swatch" style:background={channelColor(ch)}></span><ChannelLabel {ch} /></a>
              </td>
              <td class="meter-col"><Meter level={ctl.level(ch)} /></td>
              <td>
                {#if (inLayout?.has_gain ?? outLayout?.has_gain)}
                  <NumberField label="{channelName(ch)} gain" hideLabel value={s.gain} min={-72} max={12} step={0.5} digits={1} unit="dB" width="4.8em" onset={(v) => ctl.setGain(ch, v)} />
                {/if}
              </td>
              <td>
                {#if inLayout?.has_mute ?? true}
                  <button class="danger" class:on={s.mute} aria-pressed={s.mute ?? false} onclick={() => ctl.setMute(ch, !s.mute)}>{s.mute === undefined ? 'M ?' : 'M'}</button>
                {/if}
              </td>
              <td>
                {#if out}
                  <button class:on={out.invert} aria-pressed={out.invert ?? false} title="Invert polarity" onclick={() => ctl.setInvert(ch.index, !out.invert)}>Ø</button>
                {/if}
              </td>
              <td>
                {#if out && outLayout?.has_delay}
                  <NumberField label="{channelName(ch)} delay" hideLabel value={out.delayMs} min={0} max={Math.floor(ctl.layout!.max_delay_ms * 100) / 100} step={0.01} digits={2} unit="ms" width="4.8em" onset={(v) => ctl.setDelay(ch.index, v)} />
                {/if}
              </td>
              <td class="summary">{summary(ch) || '—'}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </section>

  <p class="note">
    Channel settings show what this page last sent. The device can't report them, so “—” or “?” means
    unknown, and changes made from another app won't appear here.
    <button class="ghost" onclick={() => { ctl.memory.forget(); ctl.refreshSlots(); }}>
      Forget remembered values
    </button>
  </p>
</div>

<style>
  .page {
    display: grid;
    /* minmax(0, …) lets wide tables scroll inside their panel instead of widening the page */
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }
  .plot-body {
    padding: 8px 8px 4px;
  }
  .legend {
    display: flex;
    gap: 2px;
    flex-wrap: wrap;
    justify-content: flex-end;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 6px;
  }
  .chip.off {
    opacity: 0.35;
  }
  .swatch {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 1px;
    flex: none;
  }
  .ch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--text);
    text-decoration: none;
  }
  .ch:hover {
    color: var(--accent);
  }
  .meter-col {
    min-width: 140px;
    width: 30%;
  }
  .summary {
    color: var(--muted);
    font-size: 11px;
  }
  .note {
    margin: 0;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--dim);
  }
</style>
