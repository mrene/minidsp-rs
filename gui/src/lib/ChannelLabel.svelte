<script lang="ts">
  import { channelName, ctl, type ChannelRef } from './controller.svelte';

  interface Props {
    ch: ChannelRef;
    /** Show the default name ("OUT 3") next to a custom one */
    showTag?: boolean;
  }
  let { ch, showTag = true }: Props = $props();

  const custom = $derived(ctl.customName(ch));
</script>

<span class="label-wrap" title={custom ? `${custom} (${channelName(ch)})` : undefined}>
  <span class="name" class:custom>{custom ?? channelName(ch)}</span>
  {#if custom && showTag}<span class="tag">{channelName(ch)}</span>{/if}
</span>

<style>
  .label-wrap {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name.custom {
    font-family: var(--sans);
  }
  .tag {
    flex: none;
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.05em;
    color: var(--dim);
  }
</style>
