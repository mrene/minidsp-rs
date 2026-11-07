// Central state for the selected device, and every action the interface can take on it.
import { api, latestSender, msToDuration, watchDevice, type ConnectionState } from './api';
import {
  ChannelMemory,
  type CompressorState,
  type InputState,
  type OutputState,
  type PeqBand,
  type XoverGroup,
} from './channels.svelte';
import {
  cascadeDb,
  crossoverSections,
  designBiquad,
  firDb,
  logFrequencies,
  PASSTHROUGH,
  type Coeffs,
} from './dsp';
import { reportError } from './toasts.svelte';
import type { BiquadConfig, Config, Device, DeviceLayout, MasterStatus, PeqConfig, StatusSummary } from './types';

export type ChannelKind = 'in' | 'out';
export interface ChannelRef {
  kind: ChannelKind;
  index: number;
}

export const channelName = (ch: ChannelRef) => `${ch.kind === 'in' ? 'IN' : 'OUT'} ${ch.index + 1}`;

/** Trace colors, one per output (inputs reuse them); dark enough to read on light grey */
export const PALETTE = [
  '#0a8aa3', '#c2367f', '#b07d00', '#23924f', '#d0611a', '#6a4fd6',
  '#2f6fd0', '#c8405f', '#5f8a12', '#12917f', '#a8661a', '#4d5866',
];
export const channelColor = (ch: ChannelRef) =>
  PALETTE[(ch.kind === 'out' ? ch.index : ch.index + 6) % PALETTE.length];

/** Coefficients actually loaded for a PEQ band */
export function bandCoeffs(band: PeqBand, fs: number): Coeffs {
  if (band.type === 'RAW') return band.raw ?? PASSTHROUGH;
  return designBiquad(band.type, band.freq, band.gain, band.q, fs);
}

/** Biquads loaded into a crossover group, padded to the 4 slots of the group */
export function xoverCoeffs(group: XoverGroup, fs: number): Coeffs[] {
  const sections = crossoverSections(group.kind, group.type, group.order, group.freq, fs);
  while (sections.length < 4) sections.push(PASSTHROUGH);
  return sections;
}

const toBiquad = (c: Coeffs, index?: number): BiquadConfig => ({
  ...(index === undefined ? {} : { index }),
  b0: c.b0,
  b1: c.b1,
  b2: c.b2,
  a1: c.a1,
  a2: c.a2,
});

class Controller {
  devices = $state<Device[]>([]);
  devicesLoaded = $state(false);
  selected = $state(0);
  layout = $state<DeviceLayout | undefined>();
  master = $state<MasterStatus>({});
  inputLevels = $state<number[]>([]);
  outputLevels = $state<number[]>([]);
  connection = $state<ConnectionState>('connecting');
  presetBusy = $state(false);
  problem = $state<string | undefined>();
  /** Outputs whose FIR filter is currently being loaded */
  firLoading = $state<Record<number, boolean>>({});

  memory = new ChannelMemory();

  /** Frequencies used for every response plot */
  freqs = $derived(logFrequencies(this.layout?.sampling_rate || 96000));
  fs = $derived(this.layout?.sampling_rate || 96000);

  #senders = new Map<string, (value: unknown) => void>();

  // ---- Devices and connection --------------------------------------------------------------

  async loadDevices() {
    try {
      this.devices = await api.devices();
      this.problem = this.devices.length === 0 ? 'No devices found yet. Waiting for minidspd…' : undefined;
      if (this.selected >= this.devices.length) this.selected = 0;
    } catch (e) {
      this.problem = e instanceof Error ? e.message : String(e);
    } finally {
      this.devicesLoaded = true;
    }
  }

  applySummary(summary: StatusSummary) {
    for (const [key, value] of Object.entries(summary.master ?? {})) {
      if (value !== undefined && value !== null) (this.master as Record<string, unknown>)[key] = value;
    }
    if (summary.input_levels) this.inputLevels = summary.input_levels;
    if (summary.output_levels) this.outputLevels = summary.output_levels;
  }

  /** Connects to a device: fetch its layout and status, then stream updates. Returns a cleanup. */
  connect(index: number, device: Device): () => void {
    let cancelled = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let stop: (() => void) | undefined;

    this.layout = undefined;
    this.master = {};
    this.inputLevels = [];
    this.outputLevels = [];
    this.firLoading = {};

    const attempt = async () => {
      try {
        const layout = await api.layout(index);
        if (cancelled) return;
        this.memory.use(device.url, layout.inputs.length, layout.outputs.length);
        const initial = await api.status(index);
        if (cancelled) return;
        this.applySummary(initial);
        this.layout = layout;
        this.problem = undefined;
        stop = watchDevice(index, (s) => this.applySummary(s), (s) => (this.connection = s));
      } catch (e) {
        if (cancelled) return;
        this.problem = e instanceof Error ? e.message : String(e);
        retry = setTimeout(attempt, 2000);
      }
    };
    void attempt();

    return () => {
      cancelled = true;
      clearTimeout(retry);
      stop?.();
      this.memory.flush();
    };
  }

  // ---- Sending -----------------------------------------------------------------------------

  /** One sender per control, so dragging only ever sends the latest value */
  #send<T>(key: string, value: T, request: (value: T) => Promise<unknown>) {
    const id = `${this.selected}:${key}`;
    let sender = this.#senders.get(id);
    if (!sender) {
      sender = latestSender(request as (value: unknown) => Promise<unknown>);
      this.#senders.set(id, sender);
    }
    sender(value);
  }

  /**
   * Remembers a change and sends it. If the device rejects it, `onError` resets the remembered
   * value, since we can no longer tell what the device has.
   */
  #apply<T>(key: string, value: T, config: (value: T) => Config, onError: () => void) {
    this.memory.save();
    const device = this.selected;
    this.#send(key, value, async (latest: T) => {
      try {
        await api.applyConfig(device, config(latest));
      } catch (e) {
        onError();
        this.memory.save();
        reportError(e);
      }
    });
  }

  // ---- Master ------------------------------------------------------------------------------

  setVolume(db: number) {
    this.master.volume = db;
    const device = this.selected;
    this.#send('master:volume', db, async (value: number) => {
      try {
        this.applySummary({ master: await api.setMaster(device, { volume: value }) });
      } catch (e) {
        reportError(e);
      }
    });
  }

  async setMaster(change: MasterStatus) {
    const device = this.selected;
    const switchingPreset = change.preset !== undefined;
    if (switchingPreset) this.presetBusy = true;
    try {
      this.applySummary({ master: await api.setMaster(device, change) });
    } catch (e) {
      reportError(e);
    } finally {
      if (switchingPreset) this.presetBusy = false;
    }
  }

  // ---- Channels ----------------------------------------------------------------------------

  state(ch: ChannelRef): InputState | OutputState {
    return ch.kind === 'in' ? this.memory.input(ch.index) : this.memory.output(ch.index);
  }

  #channelConfig(ch: ChannelRef, body: Record<string, unknown>): Config {
    const entry = { index: ch.index, ...body };
    return ch.kind === 'in' ? { inputs: [entry] } : { outputs: [entry] };
  }

  level(ch: ChannelRef): number | undefined {
    const layout = this.layout;
    if (!layout) return undefined;
    const channels = ch.kind === 'in' ? layout.inputs : layout.outputs;
    if (!channels[ch.index]?.has_meter) return undefined;
    // Levels only include channels that have a meter, in channel order
    const position = channels.slice(0, ch.index).filter((c) => c.has_meter).length;
    return (ch.kind === 'in' ? this.inputLevels : this.outputLevels)[position];
  }

  setGain(ch: ChannelRef, db: number) {
    const s = this.state(ch);
    s.gain = db;
    this.#apply(`${ch.kind}${ch.index}:gain`, db, (gain) => this.#channelConfig(ch, { gain }), () => (s.gain = undefined));
  }

  setMute(ch: ChannelRef, mute: boolean) {
    const s = this.state(ch);
    s.mute = mute;
    this.#apply(`${ch.kind}${ch.index}:mute`, mute, (m) => this.#channelConfig(ch, { mute: m }), () => (s.mute = undefined));
  }

  setInvert(output: number, invert: boolean) {
    const s = this.memory.output(output);
    s.invert = invert;
    this.#apply(`out${output}:invert`, invert, (v) => ({ outputs: [{ index: output, invert: v }] }), () => (s.invert = undefined));
  }

  setDelay(output: number, ms: number) {
    const s = this.memory.output(output);
    s.delayMs = ms;
    this.#apply(
      `out${output}:delay`,
      ms,
      (v) => ({ outputs: [{ index: output, delay: msToDuration(v) }] }),
      () => (s.delayMs = undefined),
    );
  }

  // ---- Routing -----------------------------------------------------------------------------

  setRoute(input: number, output: number, enabled: boolean) {
    const entry = (this.memory.input(input).routing[output] ??= {});
    entry.enabled = enabled;
    this.#apply(
      `route${input}:${output}:enable`,
      enabled,
      (v) => ({ inputs: [{ index: input, routing: [{ index: output, mute: !v }] }] }),
      () => (entry.enabled = undefined),
    );
  }

  setRouteGain(input: number, output: number, db: number) {
    const entry = (this.memory.input(input).routing[output] ??= {});
    entry.gain = db;
    this.#apply(
      `route${input}:${output}:gain`,
      db,
      (gain) => ({ inputs: [{ index: input, routing: [{ index: output, gain }] }] }),
      () => (entry.gain = undefined),
    );
  }

  // ---- Parametric EQ -----------------------------------------------------------------------

  #peqConfig(band: PeqBand, index: number): PeqConfig {
    return { index, coeff: toBiquad(bandCoeffs(band, this.fs)), bypass: band.bypass };
  }

  setPeq(ch: ChannelRef, index: number, band: PeqBand) {
    const s = this.state(ch);
    s.peq[index] = band;
    this.#apply(
      `${ch.kind}${ch.index}:peq${index}`,
      band,
      (b) => this.#channelConfig(ch, { peq: [this.#peqConfig(b, index)] }),
      () => delete s.peq[index],
    );
  }

  /** Replaces several bands in one request, e.g. after importing filters from REW */
  async setPeqs(ch: ChannelRef, bands: Record<number, PeqBand>) {
    const s = this.state(ch);
    Object.assign(s.peq, bands);
    this.memory.save();
    try {
      await api.applyConfig(
        this.selected,
        this.#channelConfig(ch, {
          peq: Object.entries(bands).map(([i, b]) => this.#peqConfig(b, Number(i))),
        }),
      );
    } catch (e) {
      for (const i of Object.keys(bands)) delete s.peq[Number(i)];
      this.memory.save();
      reportError(e);
    }
  }

  // ---- Crossover ---------------------------------------------------------------------------

  setCrossover(output: number, group: number, settings: XoverGroup) {
    const s = this.memory.output(output);
    s.xover[group] = settings;
    this.#apply(
      `out${output}:xover${group}`,
      settings,
      (g) => ({
        outputs: [
          {
            index: output,
            crossover: [
              { index: group, coeff: xoverCoeffs(g, this.fs).map((c, i) => toBiquad(c, i)), bypass: g.bypass },
            ],
          },
        ],
      }),
      () => delete s.xover[group],
    );
  }

  // ---- Compressor --------------------------------------------------------------------------

  setCompressor(output: number, change: CompressorState) {
    const s = this.memory.output(output);
    Object.assign(s.compressor, change);
    const field = Object.keys(change).join(',');
    this.#apply(
      `out${output}:comp:${field}`,
      change,
      (c) => ({ outputs: [{ index: output, compressor: c }] }),
      () => {
        for (const key of Object.keys(change)) delete s.compressor[key as keyof CompressorState];
      },
    );
  }

  // ---- FIR ---------------------------------------------------------------------------------

  /** Loads FIR taps into an output. Returns whether the device accepted them. */
  async loadFir(output: number, taps: number[], name: string): Promise<boolean> {
    const s = this.memory.output(output);
    this.firLoading[output] = true;
    try {
      await api.applyConfig(this.selected, { outputs: [{ index: output, fir: { coefficients: taps } }] });
      s.fir.taps = taps;
      s.fir.name = name;
      return true;
    } catch (e) {
      s.fir.taps = undefined;
      s.fir.name = undefined;
      reportError(e);
      return false;
    } finally {
      this.firLoading[output] = false;
      this.memory.save();
    }
  }

  /**
   * Resets the FIR filter to a unit impulse, i.e. a flat, transparent filter. (Clearing it the
   * library's way loads all-zero taps, which silences the output unless the FIR is bypassed.)
   */
  async resetFir(output: number) {
    await this.loadFir(output, [1], 'Flat (unit impulse)');
  }

  setFirBypass(output: number, bypass: boolean) {
    const s = this.memory.output(output);
    s.fir.bypass = bypass;
    this.#apply(
      `out${output}:fir:bypass`,
      bypass,
      (v) => ({ outputs: [{ index: output, fir: { bypass: v } }] }),
      () => (s.fir.bypass = undefined),
    );
  }

  // ---- Responses ---------------------------------------------------------------------------

  /** Response of one PEQ band, or undefined if the band is unknown */
  bandResponse(band: PeqBand | undefined): number[] | undefined {
    if (!band) return undefined;
    return cascadeDb([bandCoeffs(band, this.fs)], this.freqs, this.fs);
  }

  /** Combined response of all known, active PEQ bands */
  peqResponse(ch: ChannelRef): number[] {
    const sections = Object.values(this.state(ch).peq)
      .filter((b) => !b.bypass)
      .map((b) => bandCoeffs(b, this.fs));
    return cascadeDb(sections, this.freqs, this.fs);
  }

  xoverResponse(output: number): number[] {
    const sections = Object.values(this.memory.output(output).xover)
      .filter((g) => !g.bypass)
      .flatMap((g) => xoverCoeffs(g, this.fs));
    return cascadeDb(sections, this.freqs, this.fs);
  }

  // FIR responses are expensive (taps × frequencies), so compute them once per loaded filter
  #firCache = new WeakMap<number[], { freqs: number[]; response: number[] }>();

  firResponse(output: number): number[] | undefined {
    const fir = this.memory.output(output).fir;
    const taps = fir.taps;
    if (!taps?.length || fir.bypass) return undefined;
    const cached = this.#firCache.get(taps);
    if (cached && cached.freqs === this.freqs) return cached.response;
    const response = firDb(taps, this.freqs, this.fs);
    this.#firCache.set(taps, { freqs: this.freqs, response });
    return response;
  }

  /** Total filter response of a channel: PEQ + crossover + FIR (gain is not included) */
  totalResponse(ch: ChannelRef, fir?: number[]): number[] {
    const peq = this.peqResponse(ch);
    if (ch.kind === 'in') return peq;
    const xover = this.xoverResponse(ch.index);
    return peq.map((v, i) => v + xover[i] + (fir ? fir[i] : 0));
  }
}

export const ctl = new Controller();

// Persist remembered values when the page is closed
if (typeof window !== 'undefined') {
  window.addEventListener('pagehide', () => ctl.memory.flush());
}
