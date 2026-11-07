<script lang="ts">
  import { onMount } from 'svelte';
  import { api, latestSender, msToDuration, watchDevice, type ConnectionState } from './lib/api';
  import { ChannelMemory } from './lib/channels.svelte';
  import { dismiss, reportError, toasts } from './lib/toasts.svelte';
  import type { Config, Device, DeviceLayout, MasterStatus, StatusSummary } from './lib/types';
  import InputStrip from './lib/InputStrip.svelte';
  import MasterPanel from './lib/MasterPanel.svelte';
  import OutputStrip from './lib/OutputStrip.svelte';
  import RoutingMatrix from './lib/RoutingMatrix.svelte';

  let devices = $state<Device[]>([]);
  let devicesLoaded = $state(false);
  let selected = $state(0);
  let layout = $state<DeviceLayout | undefined>();
  let master = $state<MasterStatus>({});
  let inputLevels = $state<number[]>([]);
  let outputLevels = $state<number[]>([]);
  let connection = $state<ConnectionState>('connecting');
  let presetBusy = $state(false);
  let problem = $state<string | undefined>();

  const memory = new ChannelMemory();

  // ---- Devices -----------------------------------------------------------------------------

  async function loadDevices() {
    try {
      devices = await api.devices();
      problem = devices.length === 0 ? 'No devices found yet. Waiting for minidspd…' : undefined;
      if (selected >= devices.length) selected = 0;
    } catch (e) {
      problem = e instanceof Error ? e.message : String(e);
    } finally {
      devicesLoaded = true;
    }
  }

  onMount(() => {
    void loadDevices();
    // Devices can take a moment to show up after the daemon starts
    const timer = setInterval(() => {
      if (devices.length === 0) void loadDevices();
    }, 3000);
    return () => clearInterval(timer);
  });

  function applySummary(summary: StatusSummary) {
    for (const [key, value] of Object.entries(summary.master ?? {})) {
      if (value !== undefined && value !== null) (master as Record<string, unknown>)[key] = value;
    }
    if (summary.input_levels) inputLevels = summary.input_levels;
    if (summary.output_levels) outputLevels = summary.output_levels;
  }

  // Connects to the selected device: fetch its layout, then stream status and levels
  $effect(() => {
    const index = selected;
    const device = devices[index];
    if (!device) return;

    let cancelled = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let stop: (() => void) | undefined;

    layout = undefined;
    master = {};
    inputLevels = [];
    outputLevels = [];

    const connect = async () => {
      try {
        const deviceLayout = await api.layout(index);
        if (cancelled) return;
        memory.use(device.url, deviceLayout.inputs.length, deviceLayout.outputs.length);
        // Show the current status right away instead of waiting for the live stream
        const initial = await api.status(index);
        if (cancelled) return;
        applySummary(initial);
        layout = deviceLayout;
        problem = undefined;
        stop = watchDevice(index, applySummary, (state) => (connection = state));
      } catch (e) {
        if (cancelled) return;
        problem = e instanceof Error ? e.message : String(e);
        retry = setTimeout(connect, 2000);
      }
    };
    void connect();

    return () => {
      cancelled = true;
      clearTimeout(retry);
      stop?.();
    };
  });

  // ---- Sending changes ---------------------------------------------------------------------

  // One sender per control, so dragging a slider only ever sends its latest position
  const senders = new Map<string, (value: number) => void>();
  function send(key: string, value: number, request: (value: number) => Promise<unknown>) {
    const id = `${selected}:${key}`;
    let sender = senders.get(id);
    if (!sender) {
      sender = latestSender(request);
      senders.set(id, sender);
    }
    sender(value);
  }

  function setVolume(db: number) {
    master.volume = db;
    const device = selected;
    send('master:volume', db, async (value) => {
      try {
        applySummary({ master: await api.setMaster(device, { volume: value }) });
      } catch (e) {
        reportError(e);
      }
    });
  }

  async function setMaster(change: MasterStatus) {
    const device = selected;
    const switchingPreset = change.preset !== undefined;
    if (switchingPreset) presetBusy = true;
    try {
      applySummary({ master: await api.setMaster(device, change) });
    } catch (e) {
      reportError(e);
    } finally {
      if (switchingPreset) presetBusy = false;
    }
  }

  /**
   * Applies a per-channel change. The value is remembered right away; if the device rejects it,
   * the remembered value becomes unknown again since we can't tell what the device has.
   */
  function setChannel<T extends object, K extends keyof T>(
    key: string,
    target: T,
    field: K,
    value: T[K] & number,
    config: (value: number) => Config,
  ) {
    target[field] = value;
    memory.save();
    const device = selected;
    send(key, value, async (latest) => {
      try {
        await api.applyConfig(device, config(latest));
      } catch (e) {
        target[field] = undefined as T[K];
        memory.save();
        reportError(e);
      }
    });
  }

  // Booleans go through the same path, encoded as 0/1
  function setChannelFlag<T extends object, K extends keyof T>(
    key: string,
    target: T,
    field: K,
    value: boolean,
    config: (value: boolean) => Config,
  ) {
    target[field] = value as T[K];
    memory.save();
    const device = selected;
    send(key, value ? 1 : 0, async (latest) => {
      try {
        await api.applyConfig(device, config(latest === 1));
      } catch (e) {
        target[field] = undefined as T[K];
        memory.save();
        reportError(e);
      }
    });
  }

  // Levels only include channels that have a meter, in channel order
  function levelFor(levels: number[], meters: boolean[], index: number): number | undefined {
    if (!meters[index]) return undefined;
    const position = meters.slice(0, index).filter(Boolean).length;
    return levels[position];
  }

  const inputMeters = $derived(layout?.inputs.map((i) => i.has_meter) ?? []);
  const outputMeters = $derived(layout?.outputs.map((o) => o.has_meter) ?? []);
  const hasRouting = $derived(layout?.inputs.some((i) => i.routing_count > 0) ?? false);

  function deviceLabel(device: Device, index: number) {
    const name = device.product_name ?? 'Unknown device';
    const serial = device.version?.serial;
    return serial ? `${name} · ${serial}` : `${name} #${index + 1}`;
  }
</script>

<div class="app">
  <header class="topbar">
    <div class="brand">
      <img src="/favicon.svg" alt="" width="22" height="22" />
      <h1>miniDSP Control</h1>
    </div>
    <div class="topbar-controls">
      {#if devices.length > 1}
        <label class="sr-only" for="device">Device</label>
        <select id="device" bind:value={selected}>
          {#each devices as device, i (device.url)}
            <option value={i}>{deviceLabel(device, i)}</option>
          {/each}
        </select>
      {:else if devices.length === 1}
        <span class="device-name">{deviceLabel(devices[0], 0)}</span>
      {/if}
      {#if layout}
        <span class="pill {connection}" title="Live status connection">
          {connection === 'live' ? 'Live' : connection === 'connecting' ? 'Connecting' : 'Offline'}
        </span>
      {/if}
    </div>
  </header>

  {#if problem}
    <div class="banner" role="status">{problem}</div>
  {/if}

  {#if !devicesLoaded}
    <p class="empty">Loading…</p>
  {:else if layout}
    <main class="grid">
      <MasterPanel
        status={master}
        sources={layout.sources}
        {presetBusy}
        onvolume={setVolume}
        onchange={setMaster}
      />

      <section class="card">
        <header><h2>Inputs</h2></header>
        <div class="strips">
          {#each layout.inputs as input, i (i)}
            {@const state = memory.input(i)}
            <InputStrip
              index={i}
              layout={input}
              {state}
              level={levelFor(inputLevels, inputMeters, i)}
              ongain={(db) => setChannel(`in:${i}:gain`, state, 'gain', db, (gain) => ({ inputs: [{ index: i, gain }] }))}
              onmute={(mute) => setChannelFlag(`in:${i}:mute`, state, 'mute', mute, (m) => ({ inputs: [{ index: i, mute: m }] }))}
            />
          {/each}
        </div>
      </section>

      <section class="card">
        <header><h2>Outputs</h2></header>
        <div class="strips">
          {#each layout.outputs as output, i (i)}
            {@const state = memory.output(i)}
            <OutputStrip
              index={i}
              layout={output}
              {state}
              level={levelFor(outputLevels, outputMeters, i)}
              maxDelayMs={layout.max_delay_ms}
              ongain={(db) => setChannel(`out:${i}:gain`, state, 'gain', db, (gain) => ({ outputs: [{ index: i, gain }] }))}
              onmute={(mute) => setChannelFlag(`out:${i}:mute`, state, 'mute', mute, (m) => ({ outputs: [{ index: i, mute: m }] }))}
              oninvert={(invert) => setChannelFlag(`out:${i}:invert`, state, 'invert', invert, (v) => ({ outputs: [{ index: i, invert: v }] }))}
              ondelay={(ms) => setChannel(`out:${i}:delay`, state, 'delayMs', ms, (v) => ({ outputs: [{ index: i, delay: msToDuration(v) }] }))}
            />
          {/each}
        </div>
      </section>

      {#if hasRouting}
        <RoutingMatrix
          inputs={layout.inputs}
          states={layout.inputs.map((_, i) => memory.input(i))}
          outputCount={layout.outputs.length}
          onenable={(i, o, enabled) => {
            const entry = (memory.input(i).routing[o] ??= {});
            setChannelFlag(`route:${i}:${o}:enable`, entry, 'enabled', enabled, (v) => ({
              inputs: [{ index: i, routing: [{ index: o, mute: !v }] }],
            }));
          }}
          ongain={(i, o, db) => {
            const entry = (memory.input(i).routing[o] ??= {});
            setChannel(`route:${i}:${o}:gain`, entry, 'gain', db, (gain) => ({
              inputs: [{ index: i, routing: [{ index: o, gain }] }],
            }));
          }}
        />
      {/if}
    </main>

    <footer class="note">
      <p>
        Channel settings show what this page last sent. The device can't report them, so a dash or
        “?” means unknown, and changes made from another app won't appear here.
      </p>
      <button
        class="link"
        onclick={() => layout && memory.forget(layout.inputs.length, layout.outputs.length)}
      >
        Forget remembered channel values
      </button>
    </footer>
  {:else if devices.length > 0}
    <p class="empty">Connecting to the device…</p>
  {/if}

  <div class="toasts" aria-live="polite">
    {#each toasts as toast (toast.id)}
      <div class="toast" role="alert">
        <span>{toast.message}</span>
        <button class="link" aria-label="Dismiss" onclick={() => dismiss(toast.id)}>✕</button>
      </div>
    {/each}
  </div>
</div>
