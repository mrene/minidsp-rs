<script lang="ts">
  import type { InputState } from './channels.svelte';
  import type { InputLayout } from './types';

  interface Props {
    inputs: InputLayout[];
    states: InputState[];
    outputCount: number;
    onenable: (input: number, output: number, enabled: boolean) => void;
    ongain: (input: number, output: number, db: number) => void;
  }
  let { inputs, states, outputCount, onenable, ongain }: Props = $props();

  const outputs = $derived(Array.from({ length: outputCount }, (_, i) => i));
</script>

<section class="card">
  <header><h2>Routing</h2></header>
  <div class="scroll">
    <table>
      <thead>
        <tr>
          <th scope="col"><span class="sr-only">Input</span></th>
          {#each outputs as o (o)}<th scope="col">Out {o + 1}</th>{/each}
        </tr>
      </thead>
      <tbody>
        {#each inputs as input, i (i)}
          <tr>
            <th scope="row">In {i + 1}</th>
            {#each outputs as o (o)}
              <td>
                {#if o < input.routing_count}
                  {@const entry = states[i]?.routing[o] ?? {}}
                  <div class="cell">
                    <button
                      class="toggle small"
                      class:on={entry.enabled}
                      class:unknown={entry.enabled === undefined}
                      aria-pressed={entry.enabled ?? false}
                      aria-label="Route input {i + 1} to output {o + 1}"
                      onclick={() => onenable(i, o, !entry.enabled)}
                    >
                      {entry.enabled === undefined ? '?' : entry.enabled ? 'On' : 'Off'}
                    </button>
                    {#if input.routing_has_gain}
                      <input
                        type="number"
                        min="-72"
                        max="12"
                        step="0.5"
                        placeholder="dB"
                        aria-label="Gain from input {i + 1} to output {o + 1}"
                        value={entry.gain ?? ''}
                        onchange={(e) => {
                          const v = e.currentTarget.valueAsNumber;
                          if (!Number.isNaN(v)) ongain(i, o, Math.max(-72, Math.min(12, v)));
                        }}
                      />
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
  .scroll {
    overflow-x: auto;
  }
  table {
    border-collapse: collapse;
    min-width: 100%;
  }
  th,
  td {
    padding: 0.4rem;
    text-align: center;
    font-size: 0.85rem;
  }
  thead th {
    color: var(--text-muted);
    font-weight: 500;
  }
  tbody th {
    text-align: left;
    color: var(--text-muted);
    font-weight: 500;
    white-space: nowrap;
  }
  .cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
  }
  .cell input {
    width: 4.2rem;
  }
  .unknown {
    opacity: 0.7;
  }
</style>
