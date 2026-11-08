// Converts a slot's remembered settings into a preset, which the daemon can write to the device
import { msToDuration } from './api';
import type { ChannelState } from './channels.svelte';
import { bandCoeffs, toBiquad, xoverCoeffs } from './filters';
import type { CompressorConfig, Config, DeviceLayout, InputConfig, OutputConfig } from './types';

const defined = <T extends object>(o: T): T =>
  Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as T;

/** Builds the device configuration for everything known in a slot's state */
export function stateToConfig(state: ChannelState, layout: DeviceLayout, fs: number): Config {
  const inputs: InputConfig[] = [];
  layout.inputs.forEach((spec, index) => {
    const s = state.inputs[index];
    if (!s) return;
    const routing = Object.entries(s.routing ?? {})
      .filter(([o]) => Number(o) < spec.routing_count)
      .map(([o, r]) =>
        defined({
          index: Number(o),
          mute: r.enabled === undefined ? undefined : !r.enabled,
          gain: spec.routing_has_gain ? r.gain : undefined,
        }),
      )
      .filter((r) => r.mute !== undefined || r.gain !== undefined);
    const peq = Object.entries(s.peq ?? {})
      .filter(([i]) => Number(i) < spec.peq_count)
      .map(([i, b]) => ({ index: Number(i), coeff: toBiquad(bandCoeffs(b, fs)), bypass: b.bypass }));
    const entry: InputConfig = defined({
      index,
      mute: spec.has_mute ? s.mute : undefined,
      gain: spec.has_gain ? s.gain : undefined,
      routing: routing.length ? routing : undefined,
      peq: peq.length ? peq : undefined,
    });
    if (Object.keys(entry).length > 1) inputs.push(entry);
  });

  const outputs: OutputConfig[] = [];
  layout.outputs.forEach((spec, index) => {
    const s = state.outputs[index];
    if (!s) return;
    const peq = Object.entries(s.peq ?? {})
      .filter(([i]) => Number(i) < spec.peq_count)
      .map(([i, b]) => ({ index: Number(i), coeff: toBiquad(bandCoeffs(b, fs)), bypass: b.bypass }));
    const crossover = Object.entries(s.xover ?? {})
      .filter(([g]) => Number(g) < spec.crossover_groups)
      .map(([g, x]) => ({ index: Number(g), coeff: xoverCoeffs(x, fs).map((c, i) => toBiquad(c, i)), bypass: x.bypass }));
    const compressor: CompressorConfig = spec.has_compressor ? defined({ ...s.compressor }) : {};
    const fir =
      spec.has_fir && (s.fir?.taps?.length || s.fir?.bypass !== undefined)
        ? defined({ coefficients: s.fir.taps?.length ? [...s.fir.taps] : undefined, bypass: s.fir.bypass })
        : undefined;
    const entry: OutputConfig = defined({
      index,
      mute: s.mute,
      gain: spec.has_gain ? s.gain : undefined,
      invert: s.invert,
      delay: spec.has_delay && s.delayMs !== undefined ? msToDuration(s.delayMs) : undefined,
      peq: peq.length ? peq : undefined,
      crossover: crossover.length ? crossover : undefined,
      compressor: Object.keys(compressor).length ? compressor : undefined,
      fir,
    });
    if (Object.keys(entry).length > 1) outputs.push(entry);
  });

  return { inputs, outputs };
}

/** Counts the settings a preset of this state would leave untouched because they're unknown */
export function countUnknown(state: ChannelState, layout: DeviceLayout): { unknown: number; total: number } {
  let unknown = 0;
  let total = 0;
  const count = (known: boolean) => {
    total++;
    if (!known) unknown++;
  };

  layout.inputs.forEach((spec, i) => {
    const s = state.inputs[i];
    if (spec.has_mute) count(s?.mute !== undefined);
    if (spec.has_gain) count(s?.gain !== undefined);
    for (let o = 0; o < spec.routing_count; o++) {
      count(s?.routing?.[o]?.enabled !== undefined);
      if (spec.routing_has_gain) count(s?.routing?.[o]?.gain !== undefined);
    }
    for (let b = 0; b < spec.peq_count; b++) count(s?.peq?.[b] !== undefined);
  });

  layout.outputs.forEach((spec, i) => {
    const s = state.outputs[i];
    count(s?.mute !== undefined);
    count(s?.invert !== undefined);
    if (spec.has_gain) count(s?.gain !== undefined);
    if (spec.has_delay) count(s?.delayMs !== undefined);
    for (let b = 0; b < spec.peq_count; b++) count(s?.peq?.[b] !== undefined);
    for (let g = 0; g < spec.crossover_groups; g++) count(s?.xover?.[g] !== undefined);
    if (spec.has_compressor) {
      for (const k of ['bypass', 'threshold', 'ratio', 'attack', 'release'] as const) count(s?.compressor?.[k] !== undefined);
    }
    if (spec.has_fir) {
      count(!!s?.fir?.taps?.length);
      count(s?.fir?.bypass !== undefined);
    }
  });

  return { unknown, total };
}
