// Turns the interface's editable filter parameters into device coefficients
import type { PeqBand, XoverGroup } from './channels.svelte';
import { crossoverSections, designBiquad, PASSTHROUGH, type Coeffs } from './dsp';
import type { BiquadConfig } from './types';

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

export const toBiquad = (c: Coeffs, index?: number): BiquadConfig => ({
  ...(index === undefined ? {} : { index }),
  b0: c.b0,
  b1: c.b1,
  b2: c.b2,
  a1: c.a1,
  a2: c.a2,
});
