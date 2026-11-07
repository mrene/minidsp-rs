// The device can't report per-channel settings back, so the interface remembers the values it
// last sent for each device. They are kept in the browser so they survive a page reload.

export interface InputState {
  gain?: number;
  mute?: boolean;
  routing: Record<number, { enabled?: boolean; gain?: number }>;
}

export interface OutputState {
  gain?: number;
  mute?: boolean;
  invert?: boolean;
  delayMs?: number;
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

  /**
   * Switches to the remembered values of a device, identified by its URL, and makes sure every
   * channel has an entry (state must not be created while rendering).
   */
  use(deviceUrl: string, inputs: number, outputs: number) {
    this.#key = `minidsp-gui:channels:${deviceUrl}`;
    const state = load(this.#key);
    for (let i = 0; i < inputs; i++) {
      state.inputs[i] ??= { routing: {} };
      state.inputs[i].routing ??= {};
    }
    for (let i = 0; i < outputs; i++) state.outputs[i] ??= {};
    this.state = state;
  }

  input(index: number): InputState {
    return this.state.inputs[index];
  }

  output(index: number): OutputState {
    return this.state.outputs[index];
  }

  save() {
    if (!this.#key) return;
    try {
      localStorage.setItem(this.#key, JSON.stringify(this.state));
    } catch {
      // Not persisted, still kept for this session
    }
  }

  /** Clears the remembered values, e.g. after the device was changed from another app */
  forget(inputs: number, outputs: number) {
    try {
      localStorage.removeItem(this.#key);
    } catch {
      // Nothing to clear
    }
    this.use(this.#key.replace('minidsp-gui:channels:', ''), inputs, outputs);
  }
}
