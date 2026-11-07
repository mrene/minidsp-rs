<script lang="ts">
  import { onMount } from 'svelte';
  import ChannelPage from './lib/ChannelPage.svelte';
  import { ctl } from './lib/controller.svelte';
  import MasterBar from './lib/MasterBar.svelte';
  import Nav from './lib/Nav.svelte';
  import Overview from './lib/Overview.svelte';
  import { router } from './lib/route.svelte';
  import Routing from './lib/Routing.svelte';
  import { dismiss, toasts } from './lib/toasts.svelte';
  import type { Device } from './lib/types';

  onMount(() => {
    void ctl.loadDevices();
    // Devices can take a moment to show up after the daemon starts
    const timer = setInterval(() => {
      if (ctl.devices.length === 0) void ctl.loadDevices();
    }, 3000);
    return () => clearInterval(timer);
  });

  // Connect to the selected device
  $effect(() => {
    const index = ctl.selected;
    const device = ctl.devices[index];
    if (!device) return;
    return ctl.connect(index, device);
  });

  // A channel route that doesn't exist on this device falls back to the overview
  const route = $derived.by(() => {
    const r = router.route;
    const layout = ctl.layout;
    if (r.page === 'channel' && layout) {
      const count = r.ch.kind === 'in' ? layout.inputs.length : layout.outputs.length;
      if (r.ch.index >= count) return { page: 'overview' as const };
    }
    return r;
  });

  function deviceLabel(device: Device, index: number) {
    const name = device.product_name ?? 'Unknown';
    const serial = device.version?.serial;
    return serial ? `${name} · ${serial}` : `${name} #${index + 1}`;
  }
</script>

<div class="shell">
  <header class="top">
    <div class="brand">
      <span class="logo" aria-hidden="true">
        <svg viewBox="0 0 16 16" width="16" height="16"><path d="M1 12 L4 12 L6 4 L9 14 L11 8 L15 8" fill="none" stroke="var(--accent)" stroke-width="1.6" stroke-linejoin="round" /></svg>
      </span>
      <span class="name">minidsp</span>
      {#if ctl.devices.length > 1}
        <label class="sr-only" for="device">Device</label>
        <select id="device" bind:value={ctl.selected}>
          {#each ctl.devices as device, i (device.url)}
            <option value={i}>{deviceLabel(device, i)}</option>
          {/each}
        </select>
      {:else if ctl.devices.length === 1}
        <span class="device num">{deviceLabel(ctl.devices[0], 0)}</span>
      {/if}
      {#if ctl.layout}
        <span class="status {ctl.connection}" title="Live status connection">
          <span class="dot"></span>{ctl.connection === 'live' ? 'LIVE' : ctl.connection === 'connecting' ? 'CONNECTING' : 'OFFLINE'}
        </span>
      {/if}
    </div>
    {#if ctl.layout}<MasterBar />{/if}
  </header>

  {#if ctl.problem}
    <div class="banner num" role="status">{ctl.problem}</div>
  {/if}

  {#if !ctl.devicesLoaded}
    <p class="empty num">Loading…</p>
  {:else if ctl.layout}
    <div class="body">
      <aside><Nav /></aside>
      <main>
        {#if route.page === 'overview'}
          <Overview />
        {:else if route.page === 'routing'}
          <Routing />
        {:else}
          {#key `${route.ch.kind}${route.ch.index}`}
            <ChannelPage ch={route.ch} />
          {/key}
        {/if}
      </main>
    </div>
  {:else if ctl.devices.length > 0}
    <p class="empty num">Connecting to the device…</p>
  {/if}

  <div class="toasts" aria-live="polite">
    {#each toasts as toast (toast.id)}
      <div class="toast" class:info={toast.kind === 'info'} role={toast.kind === 'error' ? 'alert' : 'status'}>
        <span>{toast.message}</span>
        <button class="ghost" aria-label="Dismiss" onclick={() => dismiss(toast.id)}>✕</button>
      </div>
    {/each}
  </div>
</div>

<style>
  .shell {
    min-height: 100vh;
  }
  .top {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px 24px;
    flex-wrap: wrap;
    padding: 8px 16px;
    background: var(--header);
    backdrop-filter: blur(6px);
    border-bottom: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .logo {
    display: inline-flex;
  }
  .name {
    font-family: var(--mono);
    font-weight: 600;
    letter-spacing: 0.06em;
  }
  .device {
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  #device {
    max-width: 100%;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--dim);
  }
  .status.live {
    color: var(--ok);
  }
  .status.live .dot {
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }
  .status.offline {
    color: var(--danger);
  }
  .status.offline .dot {
    background: var(--danger);
  }
  .banner {
    margin: 12px 16px 0;
    padding: 8px 12px;
    font-size: 12px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-left: 2px solid var(--warn);
    border-radius: var(--radius);
  }
  .body {
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr);
    gap: 16px;
    padding: 16px;
    max-width: 1500px;
    margin: 0 auto;
  }
  aside {
    position: sticky;
    top: 64px;
    align-self: start;
  }
  /* Let the nav strip and wide tables scroll inside their area instead of widening the page */
  aside,
  main {
    min-width: 0;
  }
  .empty {
    text-align: center;
    color: var(--muted);
    padding: 64px 16px;
  }
  .toast.info {
    border-left-color: var(--accent);
  }
  @media (max-width: 760px) {
    .top {
      padding: 8px 16px;
    }
    .body {
      grid-template-columns: minmax(0, 1fr);
      gap: 10px;
      padding: 10px 16px 16px;
    }
    aside {
      position: static;
    }
  }
</style>
