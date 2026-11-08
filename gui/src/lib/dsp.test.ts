// Run with `npm test` (uses Node's built-in test runner and type stripping)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  biquadDb,
  cascadeDb,
  crossoverSections,
  designBiquad,
  firDb,
  parseRewBiquads,
  parseTextTaps,
  parseWav,
  type XoverKind,
} from './dsp.ts';

const fs = 96000;
const near = (actual: number, expected: number, tolerance: number) =>
  assert.ok(Math.abs(actual - expected) <= tolerance, `${actual} is not within ${tolerance} of ${expected}`);

test('peaking filter hits its gain at the center frequency', () => {
  const c = designBiquad('PK', 1000, 6, 1, fs);
  near(biquadDb(c, 1000, fs), 6, 0.01);
  near(biquadDb(c, 20, fs), 0, 0.1);
  // Device convention: feedback terms are negated, so a1 is positive for low frequencies
  assert.ok(c.a1 > 0);
});

test('shelves reach their gain away from the corner', () => {
  near(biquadDb(designBiquad('LS', 100, -4, 0.707, fs), 10, fs), -4, 0.1);
  near(biquadDb(designBiquad('HS', 5000, 3, 0.707, fs), 23000, fs), 3, 0.15);
});

test('crossovers fit in 4 sections and cross at -3 dB (BW) / -6 dB (LR)', () => {
  const cases: [XoverKind, number, number][] = [
    ['BW', 1, -3.01], ['BW', 2, -3.01], ['BW', 3, -3.01], ['BW', 4, -3.01],
    ['BW', 5, -3.01], ['BW', 6, -3.01], ['BW', 7, -3.01], ['BW', 8, -3.01],
    ['LR', 2, -6.02], ['LR', 4, -6.02], ['LR', 6, -6.02], ['LR', 8, -6.02],
  ];
  for (const [kind, order, expected] of cases) {
    for (const type of ['LP', 'HP'] as const) {
      const sections = crossoverSections(kind, type, order, 1000, fs);
      assert.ok(sections.length <= 4, `${kind}${order} needs ${sections.length} sections`);
      near(cascadeDb(sections, [1000], fs)[0], expected, 0.05);
      // Slope: order × 6 dB/octave, measured two octaves into the stop band
      const stop = cascadeDb(sections, [type === 'LP' ? 4000 : 250], fs)[0];
      near(stop, -order * 12, order * 1.2);
    }
  }
});

test('parses REW miniDSP biquad exports', () => {
  const text = readFileSync(new URL('../../../minidsp/test_fixtures/rew-filters.txt', import.meta.url), 'utf8');
  const filters = parseRewBiquads(text);
  assert.equal(filters.length, 10);
  assert.equal(filters[0].index, 1);
  assert.equal(Math.fround(filters[0].coeffs.a1), Math.fround(1.997_335_4));
  assert.equal(Math.fround(filters[9].coeffs.a2), Math.fround(-0.610_432));
});

test('FIR response of a unit impulse is flat', () => {
  for (const v of firDb([1, 0, 0, 0], [100, 1000, 10000], fs)) near(v, 0, 1e-9);
});

test('parses coefficient text files', () => {
  assert.deepEqual(parseTextTaps('# header\n0.5\n0.25, 0.125\nnot a number\n').taps, [0.5, 0.25, 0.125]);
});

test('parses 16-bit and float WAV files', () => {
  const wav = (format: number, bits: number, samples: number[]) => {
    const bytes = bits / 8;
    const buffer = new ArrayBuffer(44 + samples.length * bytes);
    const view = new DataView(buffer);
    const ascii = (at: number, s: string) => [...s].forEach((ch, i) => view.setUint8(at + i, ch.charCodeAt(0)));
    ascii(0, 'RIFF'); view.setUint32(4, 36 + samples.length * bytes, true); ascii(8, 'WAVE');
    ascii(12, 'fmt '); view.setUint32(16, 16, true); view.setUint16(20, format, true); view.setUint16(22, 1, true);
    view.setUint32(24, 96000, true); view.setUint32(28, 96000 * bytes, true); view.setUint16(32, bytes, true); view.setUint16(34, bits, true);
    ascii(36, 'data'); view.setUint32(40, samples.length * bytes, true);
    samples.forEach((s, i) => (format === 3 ? view.setFloat32(44 + i * 4, s, true) : view.setInt16(44 + i * 2, s, true)));
    return buffer;
  };
  const pcm = parseWav(wav(1, 16, [32767, 0, -16384]));
  assert.equal(pcm.sampleRate, 96000);
  near(pcm.taps[0], 1, 1e-6);
  near(pcm.taps[2], -0.5, 1e-4);
  assert.deepEqual(parseWav(wav(3, 32, [0.5, -0.25])).taps, [0.5, -0.25]);
  assert.throws(() => parseWav(new ArrayBuffer(8)));
});
