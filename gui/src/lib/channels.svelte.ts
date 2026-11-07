// The device can't report per-channel settings back, so the interface remembers the values it
// last sent for each device. They are kept in the browser so they survive a page reload.
import type { Coeffs, PeqType, XoverKind } from './dsp';

export interface PeqBand {
  type: PeqType;
  freq: number;
  gain: number;
  q: number;
  bypass: boolean;
  /** Coefficients for imported filters (type RAW) */
  raw?: Coeffs;
}

export interface XoverGroup {
  kind: XoverKind;
  type: 'LP' | 'HP';
  order: number;
  freq: number;
  bypass: boolean;
}

export interface CompressorState {
  bypass?: boolean;
  threshold?: number;
  ratio?: number;
  attack?: number;
  release?: number;
}

export interface FirState {
  bypass?: boolean;
  /** Name of the file that was loaded */
  name?: string;
  taps?: number[];
}

export interface InputState {
  gain?: number;
  mute?: boolean;
  routing: Record<number, { enabled?: boolean; gain?: number }>;
  peq: Record<number, PeqBand>;
}

export interface OutputState {
  gain?: number;
  mute?: boolean;
  invert?: boolean;
  delayMs?: number;
  peq: Record<number, PeqBand>;
  xover: Record<number, XoverGroup>;
  compressor: CompressorState;
  fir: FirState;
}

export interface ChannelState {
  inputs: Record<number, InputState>;
  outputs: Record<number, OutputState>;
}

const empty = (): ChannelState => ({ inputs: {}, outputs: {} });

function load(key: string): ChannelState {
  try {
    const raw = localStorage.getItem(key);
    if (raw) return { ...empty(), ...JSON.parse(raw) };
  } catch {
    // Storage unavailable or corrupted, start fresh
  }
  return empty();
}

export class ChannelMemory {
  state = $state<ChannelState>(empty());
  #key = '';
  #saveTimer: ReturnType<typeof setTimeout> | undefined;

  /**
   * Switches to the remembered values of a device, identified by its URL, and makes sure every
   * channel has an entry (state must not be created while rendering).
   */
  use(deviceUrl: string, inputs: number, outputs: number) {
    this.flush();
    this.#key = `minidsp-gui:channels:${deviceUrl}`;
    const state = load(this.#key);
    for (let i = 0; i < inputs; i++) {
      const input = (state.inputs[i] ??= { routing: {}, peq: {} });
      input.routing ??= {};
      input.peq ??= {};
    }
    for (let i = 0; i < outputs; i++) {
      const output = (state.outputs[i] ??= { peq: {}, xover: {}, compressor: {}, fir: {} });
      output.peq ??= {};
      output.xover ??= {};
      output.compressor ??= {};
      output.fir ??= {};
    }
    this.state = state;
  }

  input(index: number): InputState {
    return this.state.inputs[index];
  }

  output(index: number): OutputState {
    return this.state.outputs[index];
  }

  /** Persists the state shortly after the last change (dragging produces many changes) */
  save() {
    if (!this.#key) return;
    clearTimeout(this.#saveTimer);
    this.#saveTimer = setTimeout(() => this.flush(), 400);
  }

  flush() {
    if (this.#saveTimer === undefined || !this.#key) return;
    clearTimeout(this.#saveTimer);
    this.#saveTimer = undefined;
    try {
      localStorage.setItem(this.#key, JSON.stringify($state.snapshot(this.state)));
    } catch {
      // Not persisted, still kept for this session
    }
  }

  /** Clears the remembered values, e.g. after the device was changed from another app */
  forget(inputs: number, outputs: number) {
    clearTimeout(this.#saveTimer);
    this.#saveTimer = undefined;
    try {
      localStorage.removeItem(this.#key);
    } catch {
      // Nothing to clear
    }
    this.use(this.#key.replace('minidsp-gui:channels:', ''), inputs, outputs);
  }
}
