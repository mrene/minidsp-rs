// Mirrors the JSON returned by minidspd (see daemon/src/http)

export interface DeviceInfo {
  hw_id: number;
  fw_major: number;
  fw_minor: number;
  dsp_version: number;
  serial: number;
}

export interface Device {
  url: string;
  version?: DeviceInfo | null;
  product_name?: string | null;
}

/** Input source names, as serialized by the daemon (e.g. "Toslink") */
export type Source = string;

export interface MasterStatus {
  preset?: number;
  source?: Source;
  /** dB, in [-127, 0] */
  volume?: number;
  mute?: boolean;
  dirac?: boolean;
}

export interface StatusSummary {
  master: MasterStatus;
  input_levels?: number[];
  output_levels?: number[];
}

export interface InputLayout {
  has_mute: boolean;
  has_gain: boolean;
  has_meter: boolean;
  peq_count: number;
  routing_count: number;
  routing_has_gain: boolean;
}

export interface OutputLayout {
  has_gain: boolean;
  has_meter: boolean;
  has_delay: boolean;
  peq_count: number;
  has_crossover: boolean;
  /** Number of crossover groups, each holding 4 cascaded biquads */
  crossover_groups: number;
  has_compressor: boolean;
  has_fir: boolean;
  /** Maximum number of FIR taps, 0 if there is no FIR filter */
  fir_max_taps: number;
}

export interface DeviceLayout {
  product_name: string;
  sources: Source[];
  sampling_rate: number;
  max_delay_ms: number;
  inputs: InputLayout[];
  outputs: OutputLayout[];
}

/** Serde's representation of a std::time::Duration */
export interface Duration {
  secs: number;
  nanos: number;
}

export interface RoutingEntryConfig {
  index: number;
  mute?: boolean;
  gain?: number;
}

/** Biquad coefficients in the device convention (see dsp.ts) */
export interface BiquadConfig {
  index?: number;
  b0: number;
  b1: number;
  b2: number;
  a1: number;
  a2: number;
}

export interface PeqConfig {
  index: number;
  coeff?: BiquadConfig;
  bypass?: boolean;
}

export interface CrossoverConfig {
  /** Crossover group index */
  index: number;
  /** Biquads within the group, each with its own index (0-3) */
  coeff: BiquadConfig[];
  bypass?: boolean;
}

export interface CompressorConfig {
  bypass?: boolean;
  threshold?: number;
  ratio?: number;
  attack?: number;
  release?: number;
}

export interface FirConfig {
  bypass?: boolean;
  /** An empty array clears the filter */
  coefficients?: number[];
}

export interface InputConfig {
  index: number;
  mute?: boolean;
  gain?: number;
  routing?: RoutingEntryConfig[];
  peq?: PeqConfig[];
}

export interface OutputConfig {
  index: number;
  mute?: boolean;
  gain?: number;
  invert?: boolean;
  delay?: Duration;
  peq?: PeqConfig[];
  crossover?: CrossoverConfig[];
  compressor?: CompressorConfig;
  fir?: FirConfig;
}

export interface Config {
  master_status?: MasterStatus;
  inputs?: InputConfig[];
  outputs?: OutputConfig[];
}

export interface PresetSummary {
  id: string;
  name: string;
  notes: string;
  product_name: string;
  updated_ms: number;
}

export interface Preset {
  name: string;
  notes: string;
  product_name: string;
  updated_ms?: number;
  config: Config;
  /** The interface's editable parameters (ChannelState), opaque to the daemon */
  ui: unknown;
}
