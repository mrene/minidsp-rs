<script lang="ts">
  import type { XoverGroup } from './channels.svelte';
  import { ctl } from './controller.svelte';
  import { slope, XOVER_ORDERS, type XoverKind } from './dsp';
  import NumberField from './NumberField.svelte';

  interface Props {
    output: number;
    groups: number;
  }
  let { output, groups }: Props = $props();

  const state = $derived(ctl.memory.output(output).xover);
  const indexes = $derived(Array.from({ length: groups }, (_, i) => i));

  /** First group defaults to a high pass, the second to a low pass (a typical band-pass way) */
  const defaults = (g: number): XoverGroup => ({
    kind: 'LR',
    type: g === 0 ? 'HP' : 'LP',
    order: 4,
    freq: g === 0 ? 80 : 2000,
    bypass: false,
  });

  function update(g: number, change: Partial<XoverGroup>) {
    const next = { ...(state[g] ?? defaults(g)), ...change };
    // Linkwitz-Riley only exists in even orders
    if (!XOVER_ORDERS[next.kind].includes(next.order)) {
      next.order = XOVER_ORDERS[next.kind].reduce((best, o) => (Math.abs(o - next.order) < Math.abs(best - next.order) ? o : best));
    }
    ctl.setCrossover(output, g, next);
  }
</script>

<section class="panel">
  <div class="panel-head"><h2>Crossover</h2></div>
  <div class="table-scroll">
    <table class="data">
      <thead>
        <tr><th>Grp</th><th>Type</th><th title="Linkwitz-Riley or Butterworth">Align</th><th title="Per octave">Slope</th><th>Freq</th><th><span class="sr-only">Bypass</span></th></tr>
      </thead>
      <tbody>
        {#each indexes as g (g)}
          {@const group = state[g]}
          <tr class:bypassed={group?.bypass}>
            <td class="idx">{g + 1}</td>
            <td>
              <select aria-label="Group {g + 1} filter" value={group?.type ?? ''} onchange={(e) => update(g, { type: e.currentTarget.value as 'LP' | 'HP' })}>
                {#if !group}<option value="" disabled>—</option>{/if}
                <option value="HP" title="High pass">HP</option>
                <option value="LP" title="Low pass">LP</option>
              </select>
            </td>
            <td>
              <select aria-label="Group {g + 1} alignment" value={group?.kind ?? ''} disabled={!group} onchange={(e) => update(g, { kind: e.currentTarget.value as XoverKind })}>
                {#if !group}<option value="" disabled>—</option>{/if}
                <option value="LR" title="Linkwitz-Riley">LR</option>
                <option value="BW" title="Butterworth">BW</option>
              </select>
            </td>
            <td>
              <select aria-label="Group {g + 1} slope" value={group?.order ?? ''} disabled={!group} onchange={(e) => update(g, { order: Number(e.currentTarget.value) })}>
                {#if !group}<option value="" disabled>—</option>{/if}
                {#each XOVER_ORDERS[group?.kind ?? 'LR'] as order (order)}
                  <option value={order}>{slope(order)} dB</option>
                {/each}
              </select>
            </td>
            <td>
              <NumberField label="Group {g + 1} frequency" hideLabel value={group?.freq} min={10} max={ctl.fs * 0.49} step={1} digits={0} unit="Hz" width="5.6em" disabled={!group} onset={(v) => update(g, { freq: v })} />
            </td>
            <td>
              <button class="warn" class:on={group?.bypass} aria-pressed={group?.bypass ?? false} disabled={!group} onclick={() => update(g, { bypass: !group?.bypass })}>Byp</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</section>

<style>
  .idx {
    color: var(--muted);
  }
</style>
