<script lang="ts">
  import ChannelLabel from './ChannelLabel.svelte';
  import { channelColor, ctl } from './controller.svelte';
  import NumberField from './NumberField.svelte';

  const inputs = $derived(ctl.layout?.inputs ?? []);
  const outputs = $derived(Array.from({ length: ctl.layout?.outputs.length ?? 0 }, (_, i) => i));
</script>

<section class="panel">
  <div class="panel-head"><h2>Routing matrix · input → output</h2></div>
  <div class="table-scroll">
    <table class="data matrix">
      <thead>
        <tr>
          <th><span class="sr-only">Input</span></th>
          {#each outputs as o (o)}
            <th><span class="swatch" style:background={channelColor({ kind: 'out', index: o })}></span><ChannelLabel ch={{ kind: 'out', index: o }} showTag={false} /></th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each inputs as input, i (i)}
          <tr>
            <th scope="row"><span class="swatch" style:background={channelColor({ kind: 'in', index: i })}></span><ChannelLabel ch={{ kind: 'in', index: i }} /></th>
            {#each outputs as o (o)}
              <td>
                {#if o < input.routing_count}
                  {@const entry = ctl.memory.input(i).routing[o] ?? {}}
                  <div class="cell" class:active={entry.enabled}>
                    <button
                      class:on={entry.enabled}
                      aria-pressed={entry.enabled ?? false}
                      aria-label="Route input {i + 1} to output {o + 1}"
                      onclick={() => ctl.setRoute(i, o, !entry.enabled)}
                    >
                      {entry.enabled === undefined ? '?' : entry.enabled ? 'On' : 'Off'}
                    </button>
                    {#if input.routing_has_gain}
                      <NumberField label="Gain from input {i + 1} to output {o + 1}" hideLabel value={entry.gain} min={-72} max={12} step={0.5} digits={1} unit="dB" width="4.6em" onset={(v) => ctl.setRouteGain(i, o, v)} />
                    {/if}
                  </div>
                {/if}
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</section>

<style>
  .matrix th,
  .matrix td {
    text-align: center;
    padding: 8px;
  }
  .matrix tbody th {
    text-align: left;
    font-size: 11px;
    color: var(--text);
    border-bottom: 1px solid var(--grid);
  }
  .swatch {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 1px;
    margin-right: 6px;
  }
  .cell {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 6px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .cell.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
</style>
