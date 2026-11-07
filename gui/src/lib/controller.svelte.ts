// Central state for the selected device, and every action the interface can take on it.
import { api, latestSender, msToDuration, watchDevice, type ConnectionState } from './api';
import {
  ChannelMemory,
  mergeState,
  peekSlot,
  type ChannelState,
  type SlotInfo,
  type CompressorState,
  type InputState,
  type OutputState,
  type PeqBand,
  type XoverGroup,
} from './channels.svelte';
import { cascadeDb, firDb, logFrequencies } from './dsp';
import { bandCoeffs, toBiquad, xoverCoeffs } from './filters';
import { countUnknown, stateToConfig } from './presets';
import { notify, reportError } from './toasts.svelte';
import type { ChannelNames, HistoryEntry, Preset, PresetSummary, Config, Device, DeviceLayout, MasterStatus, PeqConfig, StatusSummary } from './types';

export type ChannelKind = 'in' | 'out';
export interface ChannelRef {
  kind: ChannelKind;
  index: number;
}

/** Default name of a channel, e.g. "OUT 3" */
export const channelName = (ch: ChannelRef) => `${ch.kind === 'in' ? 'IN' : 'OUT'} ${ch.index + 1}`;

/** Longest custom channel name the daemon accepts */
export const MAX_NAME_LENGTH = 32;

/** Trace colors, one per output (inputs reuse them); dark enough to read on light grey */
export const PALETTE = [
  '#0a8aa3', '#c2367f', '#b07d00', '#23924f', '#d0611a', '#6a4fd6',
  '#2f6fd0', '#c8405f', '#5f8a12', '#12917f', '#a8661a', '#4d5866',
];
export const channelColor = (ch: ChannelRef) =>
  PALETTE[(ch.kind === 'out' ? ch.index : ch.index + 6) % PALETTE.length];

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

  /** Number of configuration slots on miniDSP devices */
  readonly slotCount = 4;
  /** Slot that was active before the current one, for A/B switching */
  previousSlot = $state<number | undefined>();
  /** What each slot holds, as far as this interface knows */
  slots = $state<(SlotInfo | undefined)[]>([]);
  presets = $state<PresetSummary[]>([]);
  deletedPresets = $state<PresetSummary[]>([]);
  presetsLoaded = $state(false);
  /** Slot a preset is currently being written into */
  applyingSlot = $state<number | undefined>();

  /** Custom channel names, stored by the daemon and shared by every client */
  names = $state<ChannelNames>({ inputs: {}, outputs: {} });

  #deviceUrl = '';
  #activeSlot: number | undefined;

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
    // Each slot holds different settings, so switch the remembered values along with it
    const slot = this.master.preset;
    if (slot !== undefined && slot !== this.#activeSlot && this.layout) this.#useSlot(slot);
    if (summary.input_levels) this.inputLevels = summary.input_levels;
    if (summary.output_levels) this.outputLevels = summary.output_levels;
  }

  #useSlot(slot: number) {
    if (this.#activeSlot !== undefined) this.previousSlot = this.#activeSlot;
    this.#activeSlot = slot;
    const layout = this.layout!;
    this.memory.use(this.#deviceUrl, slot, layout.inputs.length, layout.outputs.length);
    this.refreshSlots();
  }

  /** Re-reads what every slot holds */
  refreshSlots() {
    this.memory.flush();
    this.slots = Array.from({ length: this.slotCount }, (_, i) => peekSlot(this.#deviceUrl, i));
  }

  /** Connects to a device: fetch its layout and status, then stream updates. Returns a cleanup. */
  connect(index: number, device: Device): () => void {
    let cancelled = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let stop: (() => void) | undefined;

    this.layout = undefined;
    this.#deviceUrl = device.url;
    this.#activeSlot = undefined;
    this.previousSlot = undefined;
    this.master = {};
    this.names = { inputs: {}, outputs: {} };
    this.inputLevels = [];
    this.outputLevels = [];
    this.firLoading = {};

    const attempt = async () => {
      try {
        const layout = await api.layout(index);
        if (cancelled) return;
        const initial = await api.status(index);
        // Names are optional; the interface works without them
        const names = await api.names(index).catch(() => ({ inputs: {}, outputs: {} }));
        if (cancelled) return;
        this.names = names;
        this.layout = layout;
        this.#useSlot(initial.master.preset ?? 0);
        this.applySummary(initial);
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
    this.memory.markDirty();
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

  // ---- Channel names -----------------------------------------------------------------------

  /** Custom name of a channel, if it has one */
  customName(ch: ChannelRef): string | undefined {
    return (ch.kind === 'in' ? this.names.inputs : this.names.outputs)[ch.index] || undefined;
  }

  /** Name to display: the custom name, or the default one */
  label(ch: ChannelRef): string {
    return this.customName(ch) ?? channelName(ch);
  }

  /** Renames a channel; an empty name restores the default */
  async renameChannel(ch: ChannelRef, name: string) {
    const previous = $state.snapshot(this.names) as ChannelNames;
    const next = structuredClone(previous);
    const target = ch.kind === 'in' ? next.inputs : next.outputs;
    const trimmed = name.trim().slice(0, MAX_NAME_LENGTH);
    if (trimmed) target[ch.index] = trimmed;
    else delete target[ch.index];
    this.names = next;
    try {
      this.names = await api.setNames(this.selected, next);
    } catch (e) {
      this.names = previous;
      reportError(e);
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
    this.memory.markDirty();
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
      this.memory.markDirty();
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

  // ---- Presets -----------------------------------------------------------------------------

  /** Settings in the active slot that a preset can't include, because they're unknown */
  unknownSettings() {
    return this.layout ? countUnknown(this.memory.state, this.layout) : { unknown: 0, total: 0 };
  }

  async loadPresets() {
    try {
      [this.presets, this.deletedPresets] = await Promise.all([api.presets(), api.deletedPresets()]);
    } catch (e) {
      reportError(e);
    } finally {
      this.presetsLoaded = true;
    }
  }

  #presetFromActiveSlot(name: string, notes: string): Preset {
    const state = $state.snapshot(this.memory.state) as ChannelState;
    delete state.slot;
    return {
      name,
      notes,
      product_name: this.layout!.product_name,
      config: stateToConfig(state, this.layout!, this.fs),
      ui: state,
    };
  }

  #markSlot(info: SlotInfo) {
    this.memory.state.slot = info;
    this.memory.save();
    this.refreshSlots();
  }

  /** Saves the active slot's settings as a new library preset. Returns its id. */
  async savePreset(name: string, notes: string): Promise<string | undefined> {
    try {
      const slot = (this.master.preset ?? 0) + 1;
      const { id } = await api.createPreset(this.#presetFromActiveSlot(name, notes), `created from slot ${slot}`);
      this.#markSlot({ presetId: id, presetName: id, dirty: false });
      notify(`Saved preset “${id}”`);
      await this.loadPresets();
      return id;
    } catch (e) {
      reportError(e);
    }
  }

  /** Overwrites a library preset with the active slot's settings */
  async updatePreset(id: string) {
    const existing = this.presets.find((p) => p.id === id);
    if (!existing) return;
    try {
      const slot = (this.master.preset ?? 0) + 1;
      await api.updatePreset(id, this.#presetFromActiveSlot(existing.name, existing.notes), `updated from slot ${slot}`);
      this.#markSlot({ presetId: id, presetName: existing.name, dirty: false });
      notify(`Updated preset “${existing.name}”`);
      await this.loadPresets();
    } catch (e) {
      reportError(e);
    }
  }

  async deletePreset(id: string) {
    try {
      // Slots keep naming the preset: the device still holds its settings, and the link
      // comes back if the preset is restored
      await api.deletePreset(id);
      notify(`Deleted “${id}”. It can be restored from Deleted presets.`);
      await this.loadPresets();
    } catch (e) {
      reportError(e);
    }
  }

  async restorePreset(id: string) {
    try {
      await api.restorePreset(id);
      notify(`Restored “${id}”`);
      await this.loadPresets();
    } catch (e) {
      reportError(e);
    }
  }

  /** Renames a preset. Returns the new id (the name), or undefined if it failed. */
  async renamePreset(id: string, name: string): Promise<string | undefined> {
    try {
      const { id: newId } = await api.renamePreset(id, name);
      this.#relinkSlots(id, newId);
      await this.loadPresets();
      return newId;
    } catch (e) {
      reportError(e);
    }
  }

  async setPresetNotes(id: string, notes: string) {
    try {
      const preset = await api.preset(id);
      await api.updatePreset(id, { ...preset, notes }, 'edited notes');
      await this.loadPresets();
    } catch (e) {
      reportError(e);
    }
  }

  async presetHistory(id: string): Promise<HistoryEntry[]> {
    try {
      return await api.presetHistory(id);
    } catch (e) {
      reportError(e);
      return [];
    }
  }

  /** Undoes the last change to a preset. Repeating it steps further back. */
  async undoPreset(id: string) {
    try {
      const entry = await api.undoPreset(id);
      this.#markLinkedSlotsChanged(id);
      notify(`“${id}” is back to version ${entry.restored_from}`);
      await this.loadPresets();
    } catch (e) {
      reportError(e);
    }
  }

  async rollbackPreset(id: string, version: number) {
    try {
      await api.rollbackPreset(id, version);
      this.#markLinkedSlotsChanged(id);
      notify(`Rolled “${id}” back to version ${version}`);
      await this.loadPresets();
    } catch (e) {
      reportError(e);
    }
  }

  /** Updates which preset device slots hold after a rename */
  #relinkSlots(from: string, to: string) {
    this.memory.flush();
    const relink = (state: ChannelState) => {
      if (state.slot?.presetId !== from) return false;
      state.slot = { ...state.slot, presetId: to, presetName: to };
      return true;
    };
    if (relink(this.memory.state)) this.memory.save();
    try {
      for (const key of Object.keys(localStorage)) {
        if (!key.startsWith('minidsp-gui:channels:')) continue;
        const state = JSON.parse(localStorage.getItem(key) ?? 'null') as ChannelState | null;
        if (state && relink(state)) localStorage.setItem(key, JSON.stringify(state));
      }
    } catch {
      // Storage unavailable
    }
    this.memory.use(this.#deviceUrl, this.#activeSlot ?? 0, this.layout!.inputs.length, this.layout!.outputs.length);
    this.refreshSlots();
  }

  /** After an undo or rollback, slots holding the preset no longer match it */
  #markLinkedSlotsChanged(id: string) {
    this.memory.flush();
    if (this.memory.state.slot?.presetId === id) {
      this.memory.state.slot.dirty = true;
      this.memory.save();
    }
    try {
      for (const key of Object.keys(localStorage)) {
        if (!key.startsWith('minidsp-gui:channels:')) continue;
        const state = JSON.parse(localStorage.getItem(key) ?? 'null') as ChannelState | null;
        if (state?.slot?.presetId === id) {
          state.slot.dirty = true;
          localStorage.setItem(key, JSON.stringify(state));
        }
      }
    } catch {
      // Storage unavailable
    }
    this.memory.flush();
    this.refreshSlots();
  }

  /** Switches the device to `slot` and writes a library preset into it */
  async applyPreset(id: string, slot: number) {
    const device = this.selected;
    this.applyingSlot = slot;
    try {
      const preset = await api.preset(id);
      const master = await api.applyPreset(device, id, slot);
      // The device is now on `slot`; switch to its remembered values, then overlay the preset
      this.applySummary({ master });
      if (this.#activeSlot !== slot) this.#useSlot(slot);
      const base = $state.snapshot(this.memory.state) as ChannelState;
      const merged = mergeState(base, (preset.ui ?? { inputs: {}, outputs: {} }) as ChannelState);
      merged.slot = { presetId: id, presetName: preset.name, dirty: false };
      this.memory.replace(merged);
      this.refreshSlots();
      notify(`Loaded “${preset.name}” into slot ${slot + 1}`);
    } catch (e) {
      reportError(e);
    } finally {
      this.applyingSlot = undefined;
    }
  }

  /** Flips between the current slot and the previous one */
  async toggleAB() {
    if (this.previousSlot !== undefined) await this.setMaster({ preset: this.previousSlot });
  }

  /** Downloads a library preset as a JSON file */
  async exportPreset(id: string) {
    try {
      const preset = await api.preset(id);
      const blob = new Blob([JSON.stringify(preset, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      // Exported files carry the preset's name
      a.download = `${preset.name}.json`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (e) {
      reportError(e);
    }
  }

  /** Adds a preset file (as exported) to the library */
  async importPreset(file: File) {
    try {
      const parsed = JSON.parse(await file.text()) as Partial<Preset>;
      if (typeof parsed.name !== 'string' || typeof parsed.product_name !== 'string' || typeof parsed.config !== 'object') {
        throw new Error(`${file.name} is not a minidsp preset`);
      }
      const preset: Preset = {
        name: parsed.name,
        notes: parsed.notes ?? '',
        product_name: parsed.product_name,
        config: parsed.config,
        ui: parsed.ui ?? {},
      };
      // Keep the file's name; if it's taken, add a number
      let id: string | undefined;
      for (let n = 1; !id; n++) {
        const name = n === 1 ? parsed.name : `${parsed.name} (${n})`;
        try {
          ({ id } = await api.createPreset({ ...preset, name }, `imported from ${file.name}`));
        } catch (e) {
          if (n >= 20 || !(e instanceof Error) || !e.message.includes('already exists')) throw e;
        }
      }
      notify(`Imported preset “${id}”`);
      await this.loadPresets();
    } catch (e) {
      reportError(e instanceof SyntaxError ? new Error(`${file.name} is not valid JSON`) : e);
    }
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
