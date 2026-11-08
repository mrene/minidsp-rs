// MCP server: lets an AI assistant (e.g. Claude) read and adjust a miniDSP through minidspd,
// and talk to Room EQ Wizard's API for measurement-driven EQ.
//
// Channels are numbered from 1 here (OUT 1, OUT 2…), like the web interface. Filters are
// designed at the device's own sample rate with the same code as the web interface.
import { readFile } from 'node:fs/promises';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import {
  crossoverSections,
  designBiquad,
  parseRewBiquads,
  PASSTHROUGH,
  XOVER_ORDERS,
  type Coeffs,
} from '../../gui/src/lib/dsp.ts';

const DAEMON = (process.env.MINIDSPD_URL ?? 'http://127.0.0.1:5380').replace(/\/$/, '');
const REW = (process.env.REW_API_URL ?? 'http://127.0.0.1:4735').replace(/\/$/, '');

/** Volume increases larger than this need explicit confirmation, to protect speakers and ears */
const MAX_UNCONFIRMED_VOLUME_STEP_DB = 6;

// ---- minidspd API ------------------------------------------------------------------------

class DaemonError extends Error {}

async function daemon<T>(method: string, path: string, body?: unknown): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${DAEMON}${path}`, {
      method,
      headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new DaemonError(`Cannot reach minidspd at ${DAEMON}. Is it running?`);
  }
  const text = await res.text();
  if (!res.ok) {
    let message = text || `HTTP ${res.status}`;
    try {
      const parsed = JSON.parse(text);
      if (typeof parsed?.message === 'string') message = parsed.message;
    } catch {
      // Not JSON
    }
    throw new DaemonError(message);
  }
  return (text ? JSON.parse(text) : undefined) as T;
}

interface Layout {
  product_name: string;
  sources: string[];
  sampling_rate: number;
  max_delay_ms: number;
  inputs: { has_mute: boolean; has_gain: boolean; peq_count: number; routing_count: number }[];
  outputs: {
    has_gain: boolean;
    has_delay: boolean;
    peq_count: number;
    has_crossover: boolean;
    crossover_groups: number;
    has_compressor: boolean;
    has_fir: boolean;
  }[];
}

type Kind = 'in' | 'out';
type LinkParam = 'peq' | 'crossover' | 'compressor' | 'fir' | 'gain' | 'mute' | 'delay' | 'invert';
interface Links {
  groups: { kind: Kind; channels: number[]; params: LinkParam[] }[];
}

const layout = (device: number) => daemon<Layout>('GET', `/devices/${device}/layout`);

/** 0-based channel indexes a change applies to, following channel links when asked */
async function targets(device: number, kind: Kind, channel: number, param: LinkParam, followLinks: boolean) {
  const index = channel - 1;
  if (!followLinks) return [index];
  const links = await daemon<Links>('GET', `/devices/${device}/links`).catch(() => ({ groups: [] }) as Links);
  const group = links.groups.find((g) => g.kind === kind && g.channels.includes(index));
  return group && group.params.includes(param) ? group.channels : [index];
}

function checkChannel(l: Layout, kind: Kind, channel: number) {
  const count = kind === 'in' ? l.inputs.length : l.outputs.length;
  if (!Number.isInteger(channel) || channel < 1 || channel > count) {
    throw new DaemonError(`This ${l.product_name} has ${count} ${kind === 'in' ? 'inputs' : 'outputs'}; there is no ${kind.toUpperCase()} ${channel}`);
  }
}

const biquad = (c: Coeffs, index?: number) => ({ ...(index === undefined ? {} : { index }), b0: c.b0, b1: c.b1, b2: c.b2, a1: c.a1, a2: c.a2 });

const channelConfig = (kind: Kind, entries: Record<string, unknown>[]) => (kind === 'in' ? { inputs: entries } : { outputs: entries });

const label = (kind: Kind, indexes: number[]) => indexes.map((i) => `${kind.toUpperCase()} ${i + 1}`).join(', ');

// ---- Tool plumbing -----------------------------------------------------------------------

type ToolResult = { content: { type: 'text'; text: string }[]; isError?: boolean };

const ok = (value: unknown): ToolResult => ({
  content: [{ type: 'text', text: typeof value === 'string' ? value : JSON.stringify(value, null, 2) }],
});

/** Wraps a handler so failures come back as readable tool errors instead of crashing */
function tool<A>(handler: (args: A) => Promise<ToolResult>) {
  return async (args: A): Promise<ToolResult> => {
    try {
      return await handler(args);
    } catch (e) {
      return { content: [{ type: 'text', text: e instanceof Error ? e.message : String(e) }], isError: true };
    }
  };
}

const deviceArg = z.number().int().min(0).default(0).describe('Device index from minidsp_list_devices (0 for the first device)');
const kindArg = z.enum(['in', 'out']).describe('"in" for an input channel, "out" for an output channel');
const channelArg = z.number().int().min(1).describe('Channel number, starting at 1 (OUT 1 is 1)');
const followLinksArg = z
  .boolean()
  .default(true)
  .describe('Also apply the change to channels linked with this one (as set up in the web interface)');

const server = new McpServer({ name: 'minidsp', version: '0.1.0' });

// ---- Reading -----------------------------------------------------------------------------

server.registerTool(
  'minidsp_list_devices',
  {
    title: 'List miniDSP devices',
    description: 'Lists the miniDSP devices minidspd is connected to, with their product name and serial number.',
    inputSchema: {},
  },
  tool(async () => {
    const devices = await daemon<{ url: string; product_name?: string; version?: { serial: number } }[]>('GET', '/devices');
    return ok(devices.map((d, index) => ({ device: index, product: d.product_name, serial: d.version?.serial, url: d.url })));
  }),
);

server.registerTool(
  'minidsp_get_device',
  {
    title: 'Get device state',
    description:
      "Reads a device's capabilities (channels, PEQ bands, crossover groups, sample rate), master status " +
      '(volume, mute, active preset, source), current input/output levels in dBFS, custom channel names, ' +
      'linked channels and locks. Per-channel settings (EQ, gains…) cannot be read back from miniDSP devices.',
    inputSchema: { device: deviceArg },
  },
  tool(async ({ device }: { device: number }) => {
    const [l, status, names, links, locks] = await Promise.all([
      layout(device),
      daemon<{ master: unknown; input_levels?: number[]; output_levels?: number[] }>('GET', `/devices/${device}`),
      daemon('GET', `/devices/${device}/names`).catch(() => ({})),
      daemon('GET', `/devices/${device}/links`).catch(() => ({ groups: [] })),
      daemon('GET', `/devices/${device}/locks`).catch(() => ({})),
    ]);
    return ok({
      product: l.product_name,
      filter_sample_rate_hz: l.sampling_rate,
      master: status.master,
      presets: 'Slots 1-4; "preset" in master is 0-based',
      sources: l.sources,
      input_levels_dbfs: status.input_levels,
      output_levels_dbfs: status.output_levels,
      inputs: l.inputs.map((c, i) => ({ channel: i + 1, peq_bands: c.peq_count, gain: c.has_gain, mute: c.has_mute })),
      outputs: l.outputs.map((c, i) => ({
        channel: i + 1,
        peq_bands: c.peq_count,
        crossover_groups: c.crossover_groups,
        compressor: c.has_compressor,
        fir: c.has_fir,
        delay: c.has_delay,
      })),
      max_delay_ms: l.max_delay_ms,
      names,
      links,
      locks,
    });
  }),
);

// ---- Master ------------------------------------------------------------------------------

server.registerTool(
  'minidsp_set_master',
  {
    title: 'Set master volume, mute, preset or source',
    description:
      'Changes global settings. Volume is in dB from -127 to 0. Raising the volume by more than ' +
      `${MAX_UNCONFIRMED_VOLUME_STEP_DB} dB at once requires allow_large_volume_increase, which should only be set ` +
      'after the user confirms. Preset and source changes are refused while they are locked in the web interface.',
    inputSchema: {
      device: deviceArg,
      volume_db: z.number().min(-127).max(0).optional(),
      mute: z.boolean().optional(),
      preset_slot: z.number().int().min(1).max(4).optional().describe('Configuration slot to activate, 1-4'),
      source: z.string().optional().describe('Input source name as listed by minidsp_get_device, e.g. "Toslink"'),
      allow_large_volume_increase: z.boolean().default(false),
    },
  },
  tool(async (a: { device: number; volume_db?: number; mute?: boolean; preset_slot?: number; source?: string; allow_large_volume_increase: boolean }) => {
    const change: Record<string, unknown> = {};
    if (a.volume_db !== undefined) {
      const current = (await daemon<{ master: { volume?: number } }>('GET', `/devices/${a.device}`)).master.volume;
      if (current !== undefined && a.volume_db - current > MAX_UNCONFIRMED_VOLUME_STEP_DB && !a.allow_large_volume_increase) {
        throw new DaemonError(
          `That raises the volume by ${(a.volume_db - current).toFixed(1)} dB (from ${current} to ${a.volume_db} dB). ` +
            'Ask the user to confirm, then call again with allow_large_volume_increase.',
        );
      }
      change.volume = a.volume_db;
    }
    if (a.mute !== undefined) change.mute = a.mute;
    if (a.preset_slot !== undefined) change.preset = a.preset_slot - 1;
    if (a.source !== undefined) {
      const sources = (await layout(a.device)).sources;
      const match = sources.find((s) => s.toLowerCase() === a.source!.toLowerCase());
      if (!match) throw new DaemonError(`Unknown source "${a.source}". This device has: ${sources.join(', ')}`);
      change.source = match;
    }
    if (!Object.keys(change).length) throw new DaemonError('Nothing to change');
    return ok({ master: await daemon('POST', `/devices/${a.device}`, change) });
  }),
);

// ---- Channels ----------------------------------------------------------------------------

server.registerTool(
  'minidsp_set_channel',
  {
    title: 'Set channel gain, mute, delay or polarity',
    description:
      'Sets basic settings of one input or output. Gain is in dB (-72 to +12). Delay (outputs only) is in ms. ' +
      'invert flips the polarity (outputs only).',
    inputSchema: {
      device: deviceArg,
      kind: kindArg,
      channel: channelArg,
      gain_db: z.number().min(-72).max(12).optional(),
      mute: z.boolean().optional(),
      delay_ms: z.number().min(0).optional(),
      invert: z.boolean().optional(),
      follow_links: followLinksArg,
    },
  },
  tool(async (a: { device: number; kind: Kind; channel: number; gain_db?: number; mute?: boolean; delay_ms?: number; invert?: boolean; follow_links: boolean }) => {
    const l = await layout(a.device);
    checkChannel(l, a.kind, a.channel);
    if (a.kind === 'in' && (a.delay_ms !== undefined || a.invert !== undefined)) {
      throw new DaemonError('Delay and polarity only exist on outputs');
    }
    if (a.delay_ms !== undefined && a.delay_ms > l.max_delay_ms) {
      throw new DaemonError(`Delay can be at most ${l.max_delay_ms.toFixed(2)} ms on this device`);
    }
    const applied: Record<string, string> = {};
    const fields: [keyof typeof a, LinkParam, (v: never) => Record<string, unknown>][] = [
      ['gain_db', 'gain', (v: number) => ({ gain: v })],
      ['mute', 'mute', (v: boolean) => ({ mute: v })],
      ['delay_ms', 'delay', (v: number) => ({ delay: { secs: Math.floor(v / 1000), nanos: Math.round((v % 1000) * 1e6) } })],
      ['invert', 'invert', (v: boolean) => ({ invert: v })],
    ];
    for (const [field, param, body] of fields) {
      if (a[field] === undefined) continue;
      const indexes = await targets(a.device, a.kind, a.channel, param, a.follow_links);
      await daemon('POST', `/devices/${a.device}/config`, channelConfig(a.kind, indexes.map((index) => ({ index, ...body(a[field] as never) }))));
      applied[field] = label(a.kind, indexes);
    }
    if (!Object.keys(applied).length) throw new DaemonError('Nothing to change');
    return ok({ applied_to: applied });
  }),
);

// ---- Parametric EQ -----------------------------------------------------------------------

const peqBandArg = z.object({
  band: z.number().int().min(1).describe('Band number, starting at 1'),
  type: z.enum(['PK', 'LS', 'HS', 'LP', 'HP', 'NO', 'AP']).describe('PK peak, LS/HS low/high shelf, LP/HP low/high pass, NO notch, AP all pass'),
  freq_hz: z.number().min(10).max(24000),
  gain_db: z.number().min(-24).max(24).default(0).describe('Used by PK, LS and HS'),
  q: z.number().min(0.1).max(20).default(1),
  bypass: z.boolean().default(false),
});

async function writePeq(device: number, kind: Kind, channel: number, bands: Record<number, { coeff: Coeffs; bypass: boolean }>, followLinks: boolean) {
  const l = await layout(device);
  checkChannel(l, kind, channel);
  const count = (kind === 'in' ? l.inputs : l.outputs)[channel - 1].peq_count;
  const tooHigh = Object.keys(bands).map(Number).filter((b) => b >= count);
  if (tooHigh.length) throw new DaemonError(`${kind.toUpperCase()} ${channel} has ${count} PEQ bands; band ${tooHigh[0] + 1} doesn't exist`);
  const indexes = await targets(device, kind, channel, 'peq', followLinks);
  const peq = Object.entries(bands).map(([b, v]) => ({ index: Number(b), coeff: biquad(v.coeff), bypass: v.bypass }));
  await daemon('POST', `/devices/${device}/config`, channelConfig(kind, indexes.map((index) => ({ index, peq }))));
  return { applied_to: label(kind, indexes), filter_sample_rate_hz: l.sampling_rate };
}

server.registerTool(
  'minidsp_set_peq',
  {
    title: 'Set parametric EQ bands',
    description:
      'Sets one or more parametric EQ bands of a channel. Filters are designed at the device\'s filter sample rate. ' +
      'Bands not listed are left unchanged. The web interface only shows values it set itself, so it will not reflect these.',
    inputSchema: { device: deviceArg, kind: kindArg, channel: channelArg, bands: z.array(peqBandArg).min(1), follow_links: followLinksArg },
  },
  tool(async (a: { device: number; kind: Kind; channel: number; bands: z.infer<typeof peqBandArg>[]; follow_links: boolean }) => {
    const fs = (await layout(a.device)).sampling_rate;
    const bands: Record<number, { coeff: Coeffs; bypass: boolean }> = {};
    for (const b of a.bands) bands[b.band - 1] = { coeff: designBiquad(b.type, b.freq_hz, b.gain_db, b.q, fs), bypass: b.bypass };
    return ok(await writePeq(a.device, a.kind, a.channel, bands, a.follow_links));
  }),
);

server.registerTool(
  'minidsp_flatten_peq',
  {
    title: 'Flatten parametric EQ',
    description: 'Sets every PEQ band of a channel to a flat (pass-through) filter. Useful before measuring a channel in REW.',
    inputSchema: { device: deviceArg, kind: kindArg, channel: channelArg, follow_links: followLinksArg },
  },
  tool(async (a: { device: number; kind: Kind; channel: number; follow_links: boolean }) => {
    const l = await layout(a.device);
    checkChannel(l, a.kind, a.channel);
    const count = (a.kind === 'in' ? l.inputs : l.outputs)[a.channel - 1].peq_count;
    const bands: Record<number, { coeff: Coeffs; bypass: boolean }> = {};
    for (let b = 0; b < count; b++) bands[b] = { coeff: PASSTHROUGH, bypass: false };
    return ok(await writePeq(a.device, a.kind, a.channel, bands, a.follow_links));
  }),
);

server.registerTool(
  'minidsp_load_rew_biquads',
  {
    title: 'Load REW filters into a channel',
    description:
      'Loads filters exported from Room EQ Wizard with "Save filter coefficients to file" (miniDSP biquad format: ' +
      'biquad1, b0=…, a2=…) into a channel\'s PEQ. Pass either the file path or its text. REW numbers filters from 1; ' +
      'bands not in the file are set flat. In REW, the Equalizer model must match the device\'s filter sample rate ' +
      '(e.g. Flex HTx: "Flex with Dirac Live", 48 kHz; 2x4 HD: "2x4 HD", 96 kHz), otherwise filters land at the wrong frequencies.',
    inputSchema: {
      device: deviceArg,
      kind: kindArg,
      channel: channelArg,
      file_path: z.string().optional(),
      text: z.string().optional(),
      follow_links: followLinksArg,
    },
  },
  tool(async (a: { device: number; kind: Kind; channel: number; file_path?: string; text?: string; follow_links: boolean }) => {
    const text = a.text ?? (a.file_path ? await readFile(a.file_path, 'utf8') : undefined);
    if (!text) throw new DaemonError('Pass file_path or text');
    const filters = parseRewBiquads(text);
    if (!filters.length) throw new DaemonError('No miniDSP biquads found. In REW, use "Save filter coefficients to file".');
    const l = await layout(a.device);
    checkChannel(l, a.kind, a.channel);
    const count = (a.kind === 'in' ? l.inputs : l.outputs)[a.channel - 1].peq_count;
    if (filters.some((f) => f.index > count)) {
      throw new DaemonError(`The file has filter ${Math.max(...filters.map((f) => f.index))}, but this channel has ${count} bands. Limit REW to ${count} filters.`);
    }
    const bands: Record<number, { coeff: Coeffs; bypass: boolean }> = {};
    for (let b = 0; b < count; b++) bands[b] = { coeff: PASSTHROUGH, bypass: false };
    for (const f of filters) bands[f.index - 1] = { coeff: f.coeffs, bypass: false };
    const result = await writePeq(a.device, a.kind, a.channel, bands, a.follow_links);
    return ok({ ...result, filters_loaded: filters.length, flat_bands: count - filters.length });
  }),
);

// ---- Crossover and compressor ------------------------------------------------------------

server.registerTool(
  'minidsp_set_crossover',
  {
    title: 'Set an output crossover',
    description:
      'Sets one crossover group of an output (group 1 is usually the high pass, group 2 the low pass). ' +
      'Butterworth (BW) supports 6-48 dB/octave, Linkwitz-Riley (LR) 12, 24, 36 or 48.',
    inputSchema: {
      device: deviceArg,
      channel: channelArg,
      group: z.number().int().min(1).default(1),
      type: z.enum(['LP', 'HP']),
      alignment: z.enum(['BW', 'LR']).default('LR'),
      slope_db_per_octave: z.number().int().min(6).max(48).default(24),
      freq_hz: z.number().min(10).max(24000),
      bypass: z.boolean().default(false),
      follow_links: followLinksArg,
    },
  },
  tool(async (a: { device: number; channel: number; group: number; type: 'LP' | 'HP'; alignment: 'BW' | 'LR'; slope_db_per_octave: number; freq_hz: number; bypass: boolean; follow_links: boolean }) => {
    const l = await layout(a.device);
    checkChannel(l, 'out', a.channel);
    const groups = l.outputs[a.channel - 1].crossover_groups;
    if (a.group > groups) throw new DaemonError(`OUT ${a.channel} has ${groups} crossover groups`);
    const order = a.slope_db_per_octave / 6;
    if (!XOVER_ORDERS[a.alignment].includes(order)) {
      throw new DaemonError(`${a.alignment} supports ${XOVER_ORDERS[a.alignment].map((o) => o * 6).join(', ')} dB/octave`);
    }
    const sections = crossoverSections(a.alignment, a.type, order, a.freq_hz, l.sampling_rate);
    while (sections.length < 4) sections.push(PASSTHROUGH);
    const indexes = await targets(a.device, 'out', a.channel, 'crossover', a.follow_links);
    await daemon('POST', `/devices/${a.device}/config`, {
      outputs: indexes.map((index) => ({
        index,
        crossover: [{ index: a.group - 1, coeff: sections.map((c, i) => biquad(c, i)), bypass: a.bypass }],
      })),
    });
    return ok({ applied_to: label('out', indexes), filter_sample_rate_hz: l.sampling_rate });
  }),
);

server.registerTool(
  'minidsp_set_compressor',
  {
    title: 'Set an output compressor',
    description: 'Sets compressor parameters of an output. Threshold in dBFS, attack and release in ms.',
    inputSchema: {
      device: deviceArg,
      channel: channelArg,
      bypass: z.boolean().optional(),
      threshold_dbfs: z.number().max(0).optional(),
      ratio: z.number().min(1).optional(),
      attack_ms: z.number().min(0).optional(),
      release_ms: z.number().min(0).optional(),
      follow_links: followLinksArg,
    },
  },
  tool(async (a: { device: number; channel: number; bypass?: boolean; threshold_dbfs?: number; ratio?: number; attack_ms?: number; release_ms?: number; follow_links: boolean }) => {
    const l = await layout(a.device);
    checkChannel(l, 'out', a.channel);
    const compressor = Object.fromEntries(
      Object.entries({ bypass: a.bypass, threshold: a.threshold_dbfs, ratio: a.ratio, attack: a.attack_ms, release: a.release_ms }).filter(([, v]) => v !== undefined),
    );
    if (!Object.keys(compressor).length) throw new DaemonError('Nothing to change');
    const indexes = await targets(a.device, 'out', a.channel, 'compressor', a.follow_links);
    await daemon('POST', `/devices/${a.device}/config`, { outputs: indexes.map((index) => ({ index, compressor })) });
    return ok({ applied_to: label('out', indexes) });
  }),
);

// ---- Presets -----------------------------------------------------------------------------

server.registerTool(
  'minidsp_list_presets',
  {
    title: 'List saved presets',
    description: 'Lists the preset library kept by minidspd (any number of presets, beyond the device\'s 4 slots).',
    inputSchema: {},
  },
  tool(async () => ok(await daemon('GET', '/presets'))),
);

server.registerTool(
  'minidsp_load_preset',
  {
    title: 'Load a saved preset into a slot',
    description: 'Switches the device to a slot (1-4) and writes a library preset into it. Refused while the preset is locked.',
    inputSchema: { device: deviceArg, name: z.string().describe('Preset name from minidsp_list_presets'), slot: z.number().int().min(1).max(4) },
  },
  tool(async (a: { device: number; name: string; slot: number }) =>
    ok({ master: await daemon('POST', `/devices/${a.device}/presets/${encodeURIComponent(a.name)}/apply`, { slot: a.slot - 1 }) }),
  ),
);

// ---- Room EQ Wizard ----------------------------------------------------------------------

server.registerTool(
  'rew_request',
  {
    title: 'Call the Room EQ Wizard API',
    description:
      `Sends a request to REW's local API at ${REW} (REW must be running with its API server enabled). ` +
      'Use it to list measurements, read responses, run the EQ match and fetch generated filters. Explore with GET requests ' +
      'first; REW documents its API endpoints itself. Write the generated filters to the miniDSP with minidsp_load_rew_biquads ' +
      'or minidsp_set_peq.',
    inputSchema: {
      method: z.enum(['GET', 'POST', 'PUT', 'DELETE']).default('GET'),
      path: z.string().startsWith('/').describe('Path on the REW API, e.g. /measurements'),
      body: z.unknown().optional().describe('JSON body for POST/PUT'),
    },
  },
  tool(async (a: { method: string; path: string; body?: unknown }) => {
    // Only the configured REW server: never let a path turn into a different host
    const url = new URL(a.path, `${REW}/`);
    if (url.origin !== new URL(REW).origin) throw new DaemonError('Path must stay on the REW API server');
    let res: Response;
    try {
      res = await fetch(url, {
        method: a.method,
        headers: a.body === undefined ? undefined : { 'Content-Type': 'application/json' },
        body: a.body === undefined ? undefined : JSON.stringify(a.body),
      });
    } catch {
      throw new DaemonError(`Cannot reach REW at ${REW}. Start REW and enable its API server.`);
    }
    const text = await res.text();
    const shown = text.length > 20000 ? `${text.slice(0, 20000)}\n… (${text.length - 20000} more characters)` : text;
    return ok(`HTTP ${res.status}\n${shown}`);
  }),
);

await server.connect(new StdioServerTransport());
