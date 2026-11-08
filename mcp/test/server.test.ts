// End-to-end test: launches the MCP server over stdio and calls its tools against a running
// minidspd. Start the daemon with mock devices first:
//   cargo run --bin minidspd -- --config gui/dev/minidspd.toml
import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const DAEMON = process.env.MINIDSPD_URL ?? 'http://127.0.0.1:5380';
const server = fileURLToPath(new URL('../src/server.ts', import.meta.url));
const fixture = fileURLToPath(new URL('../../minidsp/test_fixtures/rew-filters.txt', import.meta.url));
const client = new Client({ name: 'test', version: '0' });

type Result = { content: { type: string; text: string }[]; isError?: boolean };
const call = async (name: string, args: Record<string, unknown> = {}) => {
  const r = (await client.callTool({ name, arguments: args })) as Result;
  const text = r.content[0].text;
  let json: any;
  try {
    json = JSON.parse(text);
  } catch {
    json = undefined;
  }
  return { error: !!r.isError, text, json };
};
const put = (path: string, body: unknown) =>
  fetch(`${DAEMON}${path}`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });

before(async () => {
  await client.connect(new StdioClientTransport({ command: process.execPath, args: [server], env: { ...process.env, REW_API_URL: 'http://127.0.0.1:59999' } as Record<string, string> }));
});
after(async () => {
  await put('/devices/0/links', { groups: [] });
  await client.close();
});

test('lists its tools', async () => {
  const { tools } = await client.listTools();
  const names = tools.map((t) => t.name);
  for (const n of ['minidsp_list_devices', 'minidsp_get_device', 'minidsp_set_master', 'minidsp_set_peq', 'minidsp_load_rew_biquads', 'minidsp_set_crossover', 'rew_request']) {
    assert.ok(names.includes(n), n);
  }
});

test('reads devices and their state', async () => {
  const devices = await call('minidsp_list_devices');
  assert.equal(devices.json[0].product, '2x4HD');
  const state = await call('minidsp_get_device', { device: 0 });
  assert.equal(state.json.filter_sample_rate_hz, 96000);
  assert.equal(state.json.outputs.length, 4);
  assert.ok(Array.isArray(state.json.output_levels_dbfs));
});

test('guards large volume increases', async () => {
  await call('minidsp_set_master', { volume_db: -40 });
  const refused = await call('minidsp_set_master', { volume_db: -10 });
  assert.ok(refused.error && /raises the volume by 30\.0 dB/.test(refused.text), refused.text);
  const small = await call('minidsp_set_master', { volume_db: -36 });
  assert.equal(small.json.master.volume, -36);
  const confirmed = await call('minidsp_set_master', { volume_db: -3.5, allow_large_volume_increase: true });
  assert.equal(confirmed.json.master.volume, -3.5);
});

test('respects the preset lock', async () => {
  await put('/devices/0/locks', { source: true, preset: true });
  const r = await call('minidsp_set_master', { preset_slot: 2 });
  assert.ok(r.error && /locked/.test(r.text), r.text);
});

test('sets PEQ bands, following links', async () => {
  const single = await call('minidsp_set_peq', { kind: 'out', channel: 1, bands: [{ band: 1, type: 'PK', freq_hz: 100, gain_db: -4, q: 2 }] });
  assert.equal(single.json.applied_to, 'OUT 1');
  await put('/devices/0/links', { groups: [{ kind: 'out', channels: [0, 1], params: ['peq'] }] });
  const linked = await call('minidsp_set_peq', { kind: 'out', channel: 2, bands: [{ band: 3, type: 'HS', freq_hz: 6000, gain_db: -2, q: 0.7 }] });
  assert.equal(linked.json.applied_to, 'OUT 1, OUT 2');
  const unlinked = await call('minidsp_set_peq', { kind: 'out', channel: 2, follow_links: false, bands: [{ band: 3, type: 'HS', freq_hz: 6000 }] });
  assert.equal(unlinked.json.applied_to, 'OUT 2');
  // Gain isn't shared by that link
  const gain = await call('minidsp_set_channel', { kind: 'out', channel: 1, gain_db: -3 });
  assert.equal(gain.json.applied_to.gain_db, 'OUT 1');
});

test('loads REW biquad exports', async () => {
  const r = await call('minidsp_load_rew_biquads', { kind: 'in', channel: 1, file_path: fixture });
  assert.equal(r.json.filters_loaded, 10);
  assert.equal(r.json.flat_bands, 0);
  const bad = await call('minidsp_load_rew_biquads', { kind: 'in', channel: 1, text: 'not filters' });
  assert.ok(bad.error && /Save filter coefficients to file/.test(bad.text));
});

test('sets crossovers and validates slopes', async () => {
  const r = await call('minidsp_set_crossover', { channel: 3, type: 'HP', alignment: 'LR', slope_db_per_octave: 36, freq_hz: 80 });
  assert.equal(r.json.applied_to, 'OUT 3');
  const bad = await call('minidsp_set_crossover', { channel: 3, type: 'HP', alignment: 'LR', slope_db_per_octave: 18, freq_hz: 80 });
  assert.ok(bad.error && /LR supports 12, 24, 36, 48/.test(bad.text), bad.text);
});

test('validates channels', async () => {
  const r = await call('minidsp_set_channel', { kind: 'out', channel: 9, mute: true });
  assert.ok(r.error && /has 4 outputs; there is no OUT 9/.test(r.text), r.text);
  const delay = await call('minidsp_set_channel', { kind: 'in', channel: 1, delay_ms: 2 });
  assert.ok(delay.error && /only exist on outputs/.test(delay.text));
});

test('REW requests stay on the REW server', async () => {
  const off = await call('rew_request', { path: '//example.com/steal' });
  assert.ok(off.error && /must stay on the REW API server/.test(off.text), off.text);
  const down = await call('rew_request', { path: '/measurements' });
  assert.ok(down.error && /Cannot reach REW/.test(down.text), down.text);
});
