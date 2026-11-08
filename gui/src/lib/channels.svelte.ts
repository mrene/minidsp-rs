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

/** Which library preset a device slot holds, as far as this interface knows */
export interface SlotInfo {
  presetId?: string;
  presetName?: string;
  /** Changed since the preset was loaded or saved */
  dirty?: boolean;
}

export interface ChannelState {
  inputs: Record<number, InputState>;
  outputs: Record<number, OutputState>;
  slot?: SlotInfo;
}

/** Storage key for a device's configuration slot. Each slot holds different settings. */
export const slotKey = (deviceUrl: string, slot: number) => `minidsp-gui:channels:${deviceUrl}:slot${slot}`;
const legacyKey = (deviceUrl: string) => `minidsp-gui:channels:${deviceUrl}`;

/** Reads what a slot holds without switching to it */
export function peekSlot(deviceUrl: string, slot: number): SlotInfo | undefined {
  return load(slotKey(deviceUrl, slot)).slot;
}

/**
 * Overlays known values from `overlay` onto `base`. Used when a preset is loaded: the preset only
 * writes what it knows, so anything else the slot already had stays true.
 */
export function mergeState(base: ChannelState, overlay: ChannelState): ChannelState {
  const result: ChannelState = structuredClone(base);
  const assignDefined = <T extends object>(target: T, source: Partial<T> | undefined) => {
    for (const [k, v] of Object.entries(source ?? {})) {
      if (v !== undefined && (typeof v !== 'object' || Array.isArray(v))) (target as Record<string, unknown>)[k] = v;
    }
  };
  for (const [i, input] of Object.entries(overlay.inputs ?? {})) {
    const target = (result.inputs[Number(i)] ??= { routing: {}, peq: {} });
    assignDefined(target, input);
    for (const [o, r] of Object.entries(input.routing ?? {})) assignDefined((target.routing[Number(o)] ??= {}), r);
    Object.assign(target.peq, structuredClone(input.peq ?? {}));
  }
  for (const [i, output] of Object.entries(overlay.outputs ?? {})) {
    const target = (result.outputs[Number(i)] ??= { peq: {}, xover: {}, compressor: {}, fir: {} });
    assignDefined(target, output);
    Object.assign(target.peq, structuredClone(output.peq ?? {}));
    Object.assign(target.xover, structuredClone(output.xover ?? {}));
    assignDefined(target.compressor, output.compressor);
    if (output.fir?.taps) {
      target.fir.taps = [...output.fir.taps];
      target.fir.name = output.fir.name;
    }
    if (output.fir?.bypass !== undefined) target.fir.bypass = output.fir.bypass;
  }
  return result;
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

  #counts = { inputs: 0, outputs: 0 };

  /**
   * Switches to the remembered values of one of a device's slots, and makes sure every channel
   * has an entry (state must not be created while rendering).
   */
  use(deviceUrl: string, slot: number, inputs: number, outputs: number) {
    this.flush();
    this.#key = slotKey(deviceUrl, slot);
    this.#counts = { inputs, outputs };
    let state = load(this.#key);
    // Values remembered before slots were tracked belong to whichever slot is active now
    try {
      const legacy = localStorage.getItem(legacyKey(deviceUrl));
      if (legacy && !localStorage.getItem(this.#key)) {
        state = load(legacyKey(deviceUrl));
        localStorage.setItem(this.#key, legacy);
        localStorage.removeItem(legacyKey(deviceUrl));
      }
    } catch {
      // Storage unavailable
    }
    this.#prepare(state);
    this.state = state;
  }

  /** Replaces the state of the current slot, e.g. after loading a preset into it */
  replace(state: ChannelState) {
    this.#prepare(state);
    this.state = state;
    this.save();
  }

  #prepare(state: ChannelState) {
    const { inputs, outputs } = this.#counts;
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
  }

  /** Marks the slot as changed since its preset was loaded */
  markDirty() {
    if (this.state.slot?.presetId && !this.state.slot.dirty) this.state.slot.dirty = true;
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

  /** Clears the remembered values of the current slot, e.g. after it was changed from another app */
  forget() {
    clearTimeout(this.#saveTimer);
    this.#saveTimer = undefined;
    try {
      localStorage.removeItem(this.#key);
    } catch {
      // Nothing to clear
    }
    const state = empty();
    this.#prepare(state);
    this.state = state;
  }
}
