// Filter design and analysis.
//
// Coefficients use the miniDSP / REW convention, where the feedback terms are negated:
//   H(z) = (b0 + b1·z⁻¹ + b2·z⁻²) / (1 − a1·z⁻¹ − a2·z⁻²)
// This matches `Biquad` in the Rust library, so they can be sent to the device as-is.

export interface Coeffs {
  b0: number;
  b1: number;
  b2: number;
  a1: number;
  a2: number;
}

export const PASSTHROUGH: Coeffs = { b0: 1, b1: 0, b2: 0, a1: 0, a2: 0 };

export type PeqType = 'PK' | 'LS' | 'HS' | 'LP' | 'HP' | 'NO' | 'AP' | 'RAW';

export const PEQ_TYPES: { id: PeqType; label: string; hasGain: boolean; hasQ: boolean }[] = [
  { id: 'PK', label: 'Peak', hasGain: true, hasQ: true },
  { id: 'LS', label: 'Low shelf', hasGain: true, hasQ: true },
  { id: 'HS', label: 'High shelf', hasGain: true, hasQ: true },
  { id: 'LP', label: 'Low pass', hasGain: false, hasQ: true },
  { id: 'HP', label: 'High pass', hasGain: false, hasQ: true },
  { id: 'NO', label: 'Notch', hasGain: false, hasQ: true },
  { id: 'AP', label: 'All pass', hasGain: false, hasQ: true },
  { id: 'RAW', label: 'Imported', hasGain: false, hasQ: false },
];

export const peqTypeInfo = (type: PeqType) => PEQ_TYPES.find((t) => t.id === type)!;

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Converts textbook coefficients (with a0) into the device convention */
function normalize(b0: number, b1: number, b2: number, a0: number, a1: number, a2: number): Coeffs {
  return { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: -a1 / a0, a2: -a2 / a0 };
}

/** Designs a biquad using the Audio EQ Cookbook (R. Bristow-Johnson) formulas */
export function designBiquad(
  type: Exclude<PeqType, 'RAW'>,
  freq: number,
  gainDb: number,
  q: number,
  fs: number,
): Coeffs {
  const f = clamp(freq, 1, fs * 0.49);
  const w0 = (2 * Math.PI * f) / fs;
  const cos = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * Math.max(q, 0.01));
  const A = Math.pow(10, gainDb / 40);

  switch (type) {
    case 'PK':
      return normalize(1 + alpha * A, -2 * cos, 1 - alpha * A, 1 + alpha / A, -2 * cos, 1 - alpha / A);
    case 'LS': {
      const s = 2 * Math.sqrt(A) * alpha;
      return normalize(
        A * (A + 1 - (A - 1) * cos + s),
        2 * A * (A - 1 - (A + 1) * cos),
        A * (A + 1 - (A - 1) * cos - s),
        A + 1 + (A - 1) * cos + s,
        -2 * (A - 1 + (A + 1) * cos),
        A + 1 + (A - 1) * cos - s,
      );
    }
    case 'HS': {
      const s = 2 * Math.sqrt(A) * alpha;
      return normalize(
        A * (A + 1 + (A - 1) * cos + s),
        -2 * A * (A - 1 + (A + 1) * cos),
        A * (A + 1 + (A - 1) * cos - s),
        A + 1 - (A - 1) * cos + s,
        2 * (A - 1 - (A + 1) * cos),
        A + 1 - (A - 1) * cos - s,
      );
    }
    case 'LP':
      return normalize((1 - cos) / 2, 1 - cos, (1 - cos) / 2, 1 + alpha, -2 * cos, 1 - alpha);
    case 'HP':
      return normalize((1 + cos) / 2, -(1 + cos), (1 + cos) / 2, 1 + alpha, -2 * cos, 1 - alpha);
    case 'NO':
      return normalize(1, -2 * cos, 1, 1 + alpha, -2 * cos, 1 - alpha);
    case 'AP':
      return normalize(1 - alpha, -2 * cos, 1 + alpha, 1 + alpha, -2 * cos, 1 - alpha);
  }
}

/** First-order low or high pass (bilinear transform), stored in a biquad */
function firstOrder(type: 'LP' | 'HP', freq: number, fs: number): Coeffs {
  const k = Math.tan((Math.PI * clamp(freq, 1, fs * 0.49)) / fs);
  const a0 = k + 1;
  const a1 = k - 1;
  return type === 'LP'
    ? normalize(k, k, 0, a0, a1, 0)
    : normalize(1, -1, 0, a0, a1, 0);
}

export type XoverKind = 'BW' | 'LR';

export const XOVER_ORDERS: Record<XoverKind, number[]> = {
  BW: [1, 2, 3, 4, 5, 6, 7, 8],
  LR: [2, 4, 6, 8],
};

/** Slope in dB/octave for a given filter order */
export const slope = (order: number) => order * 6;

/**
 * Designs a crossover filter as cascaded sections (at most 4, the size of a crossover group).
 * Butterworth of order n uses Q = 1 / (2·sin((2k−1)·π / 2n)) per pole pair, plus a first-order
 * section for odd orders. Linkwitz-Riley of order 2n is two cascaded Butterworth filters of order n.
 */
export function crossoverSections(
  kind: XoverKind,
  type: 'LP' | 'HP',
  order: number,
  freq: number,
  fs: number,
): Coeffs[] {
  const butterworth = (n: number): Coeffs[] => {
    const sections: Coeffs[] = [];
    for (let k = 1; k <= Math.floor(n / 2); k++) {
      const q = 1 / (2 * Math.sin(((2 * k - 1) * Math.PI) / (2 * n)));
      sections.push(designBiquad(type, freq, 0, q, fs));
    }
    if (n % 2 === 1) sections.push(firstOrder(type, freq, fs));
    return sections;
  };

  if (kind === 'BW') return butterworth(order);
  const half = butterworth(order / 2);
  return [...half, ...butterworth(order / 2)];
}

/** Magnitude response of a biquad in dB at a given frequency */
export function biquadDb(c: Coeffs, freq: number, fs: number): number {
  const w = (2 * Math.PI * freq) / fs;
  const c1 = Math.cos(w);
  const s1 = Math.sin(w);
  const c2 = Math.cos(2 * w);
  const s2 = Math.sin(2 * w);
  // z⁻¹ = cos(w) − j·sin(w)
  const nr = c.b0 + c.b1 * c1 + c.b2 * c2;
  const ni = -(c.b1 * s1 + c.b2 * s2);
  const dr = 1 - c.a1 * c1 - c.a2 * c2;
  const di = c.a1 * s1 + c.a2 * s2;
  const num = nr * nr + ni * ni;
  const den = dr * dr + di * di;
  if (num <= 0) return -200;
  return 10 * Math.log10(num / den);
}

/** Magnitude response of an FIR filter in dB, evaluated at each frequency */
export function firDb(taps: number[], freqs: number[], fs: number): number[] {
  return freqs.map((f) => {
    const w = (2 * Math.PI * f) / fs;
    let re = 0;
    let im = 0;
    for (let n = 0; n < taps.length; n++) {
      re += taps[n] * Math.cos(w * n);
      im -= taps[n] * Math.sin(w * n);
    }
    const mag = re * re + im * im;
    return mag <= 0 ? -200 : 10 * Math.log10(mag);
  });
}

/** Logarithmically spaced frequencies for plotting */
export function logFrequencies(fs: number, points = 240, from = 10, to = 24000): number[] {
  const top = Math.min(to, fs * 0.49);
  const ratio = Math.log(top / from);
  return Array.from({ length: points }, (_, i) => from * Math.exp((ratio * i) / (points - 1)));
}

/** Sums the response of several biquads (cascaded filters add in dB) */
export function cascadeDb(sections: Coeffs[], freqs: number[], fs: number): number[] {
  return freqs.map((f) => sections.reduce((sum, c) => sum + biquadDb(c, f, fs), 0));
}

// ---- File formats ------------------------------------------------------------------------

/**
 * Parses REW's "Export filter settings as miniDSP biquads" text:
 *   biquad1,
 *   b0=0.99919,
 *   ...
 */
export function parseRewBiquads(text: string): { index: number; coeffs: Coeffs }[] {
  const pattern =
    /biquad(\d+),?\s*b0=([-+\d.eE]+),?\s*b1=([-+\d.eE]+),?\s*b2=([-+\d.eE]+),?\s*a1=([-+\d.eE]+),?\s*a2=([-+\d.eE]+)/g;
  const filters: { index: number; coeffs: Coeffs }[] = [];
  for (const m of text.matchAll(pattern)) {
    const [b0, b1, b2, a1, a2] = m.slice(2, 7).map(Number);
    if ([b0, b1, b2, a1, a2].some((v) => !Number.isFinite(v))) continue;
    filters.push({ index: Number(m[1]), coeffs: { b0, b1, b2, a1, a2 } });
  }
  return filters;
}

export interface ImpulseFile {
  taps: number[];
  /** Sample rate declared by the file, if it has one */
  sampleRate?: number;
}

/** Reads the first channel of a PCM (8/16/24/32-bit) or 32-bit float WAV file */
export function parseWav(buffer: ArrayBuffer): ImpulseFile {
  const view = new DataView(buffer);
  const text = (offset: number, length: number) =>
    String.fromCharCode(...new Uint8Array(buffer, offset, length));

  if (buffer.byteLength < 12 || text(0, 4) !== 'RIFF' || text(8, 4) !== 'WAVE') {
    throw new Error('Not a WAV file');
  }

  let format = 0;
  let channels = 0;
  let sampleRate = 0;
  let bits = 0;
  let offset = 12;
  while (offset + 8 <= buffer.byteLength) {
    const id = text(offset, 4);
    const size = view.getUint32(offset + 4, true);
    const body = offset + 8;

    if (id === 'fmt ') {
      format = view.getUint16(body, true);
      channels = view.getUint16(body + 2, true);
      sampleRate = view.getUint32(body + 4, true);
      bits = view.getUint16(body + 14, true);
      // WAVE_FORMAT_EXTENSIBLE: the actual format is the first 2 bytes of the sub-format GUID
      if (format === 0xfffe && size >= 26) format = view.getUint16(body + 24, true);
    } else if (id === 'data') {
      if (!format) throw new Error('WAV file has no format chunk before its data');
      const bytes = bits / 8;
      const frame = bytes * channels;
      const count = Math.floor(Math.min(size, buffer.byteLength - body) / frame);
      const taps: number[] = new Array(count);
      for (let i = 0; i < count; i++) {
        const at = body + i * frame;
        if (format === 3 && bits === 32) taps[i] = view.getFloat32(at, true);
        else if (format === 3 && bits === 64) taps[i] = view.getFloat64(at, true);
        else if (format === 1 && bits === 8) taps[i] = (view.getUint8(at) - 128) / 128;
        else if (format === 1 && bits === 16) taps[i] = view.getInt16(at, true) / 32767;
        else if (format === 1 && bits === 24) {
          const v = view.getUint8(at) | (view.getUint8(at + 1) << 8) | (view.getInt8(at + 2) << 16);
          taps[i] = v / 8388608;
        } else if (format === 1 && bits === 32) taps[i] = view.getInt32(at, true) / 2147483648;
        else throw new Error(`Unsupported WAV encoding (format ${format}, ${bits} bits)`);
      }
      if (count === 0) throw new Error('WAV file contains no samples');
      return { taps, sampleRate };
    }
    offset = body + size + (size % 2);
  }
  throw new Error('WAV file has no data');
}

/** Reads coefficients from text, one or more numbers per line; other lines are ignored */
export function parseTextTaps(text: string): ImpulseFile {
  const taps: number[] = [];
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || /^[#;*/]/.test(trimmed)) continue;
    for (const token of trimmed.split(/[\s,;]+/)) {
      const value = Number(token);
      if (token && Number.isFinite(value)) taps.push(value);
    }
  }
  if (taps.length === 0) throw new Error('No coefficients found in the file');
  return { taps };
}
