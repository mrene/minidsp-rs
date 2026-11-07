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
  has_compressor: boolean;
  has_fir: boolean;
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

export interface InputConfig {
  index: number;
  mute?: boolean;
  gain?: number;
  routing?: RoutingEntryConfig[];
}

export interface OutputConfig {
  index: number;
  mute?: boolean;
  gain?: number;
  invert?: boolean;
  delay?: Duration;
}

export interface Config {
  master_status?: MasterStatus;
  inputs?: InputConfig[];
  outputs?: OutputConfig[];
}
