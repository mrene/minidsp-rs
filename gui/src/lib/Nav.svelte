<script lang="ts">
  import { channelColor, channelName, ctl, type ChannelRef } from './controller.svelte';
  import Meter from './Meter.svelte';
  import { href, router } from './route.svelte';

  const inputs = $derived<ChannelRef[]>((ctl.layout?.inputs ?? []).map((_, index) => ({ kind: 'in', index })));
  const outputs = $derived<ChannelRef[]>((ctl.layout?.outputs ?? []).map((_, index) => ({ kind: 'out', index })));
  const hasRouting = $derived(ctl.layout?.inputs.some((i) => i.routing_count > 0) ?? false);
</script>

{#snippet channel(ch: ChannelRef)}
  <a href={href.channel(ch)} class="item ch" class:current={router.isChannel(ch)} aria-current={router.isChannel(ch) ? 'page' : undefined}>
    <span class="swatch" style:background={channelColor(ch)}></span>
    <span class="name">{channelName(ch)}</span>
    {#if ctl.state(ch)?.mute}<span class="flag">M</span>{/if}
    <span class="mini"><Meter level={ctl.level(ch)} showValue={false} /></span>
  </a>
{/snippet}

<nav aria-label="Sections">
  <a href={href.overview} class="item" class:current={router.route.page === 'overview'} aria-current={router.route.page === 'overview' ? 'page' : undefined}>Overview</a>
  {#if hasRouting}
    <a href={href.routing} class="item" class:current={router.route.page === 'routing'} aria-current={router.route.page === 'routing' ? 'page' : undefined}>Routing</a>
  {/if}
  <a href={href.presets} class="item" class:current={router.route.page === 'presets'} aria-current={router.route.page === 'presets' ? 'page' : undefined}>Presets</a>
  <span class="section label">Inputs</span>
  {#each inputs as ch (ch.index)}{@render channel(ch)}{/each}
  <span class="section label">Outputs</span>
  {#each outputs as ch (ch.index)}{@render channel(ch)}{/each}
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .section {
    padding: 14px 10px 4px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: var(--radius);
    color: var(--text);
    text-decoration: none;
    font-family: var(--mono);
    font-size: 12px;
    border-left: 2px solid transparent;
  }
  .item:hover {
    background: var(--panel-2);
  }
  .item.current {
    background: var(--raised);
    border-left-color: var(--accent);
  }
  .swatch {
    width: 8px;
    height: 8px;
    border-radius: 1px;
    flex: none;
  }
  .name {
    flex: none;
  }
  .flag {
    font-size: 9.5px;
    color: var(--danger);
    border: 1px solid currentColor;
    border-radius: 2px;
    padding: 0 3px;
  }
  .mini {
    flex: 1;
    min-width: 30px;
  }

  /* Phones: the navigation becomes a horizontal strip */
  @media (max-width: 760px) {
    nav {
      flex-direction: row;
      overflow-x: auto;
      gap: 4px;
      padding-bottom: 4px;
      scrollbar-width: none;
    }
    .section {
      display: none;
    }
    .item {
      flex: none;
      border-left: none;
      border: 1px solid var(--line);
    }
    .item.current {
      border-color: var(--accent);
    }
    .mini {
      display: none;
    }
  }
</style>
