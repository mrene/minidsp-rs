<script lang="ts">
  import ChannelLabel from './ChannelLabel.svelte';
  import { ctl, type ChannelRef } from './controller.svelte';
  import type { LinkParam } from './types';

  let { ch }: { ch: ChannelRef } = $props();

  const PARAMS: { id: LinkParam; label: string; outputsOnly?: boolean; defaultOn: boolean }[] = [
    { id: 'peq', label: 'Parametric EQ', defaultOn: true },
    { id: 'crossover', label: 'Crossover', outputsOnly: true, defaultOn: true },
    { id: 'compressor', label: 'Compressor', outputsOnly: true, defaultOn: true },
    { id: 'fir', label: 'FIR', outputsOnly: true, defaultOn: true },
    { id: 'gain', label: 'Gain', defaultOn: true },
    { id: 'mute', label: 'Mute', defaultOn: true },
    // Left and right usually need their own time alignment and polarity
    { id: 'delay', label: 'Delay', outputsOnly: true, defaultOn: false },
    { id: 'invert', label: 'Polarity', outputsOnly: true, defaultOn: false },
  ];

  const group = $derived(ctl.linkGroup(ch));
  const params = $derived(PARAMS.filter((p) => ch.kind === 'out' || !p.outputsOnly));
  const others = $derived(
    Array.from({ length: (ch.kind === 'in' ? ctl.layout?.inputs.length : ctl.layout?.outputs.length) ?? 0 }, (_, i) => i).filter(
      (i) => i !== ch.index,
    ),
  );
  const members = $derived(group ? group.channels.filter((i) => i !== ch.index) : []);

  let editing = $state(false);
  let chosen = $state<Record<number, boolean>>({});
  let shared = $state<Record<string, boolean>>({});
  let copy = $state(true);
  let saving = $state(false);

  function startEdit() {
    chosen = Object.fromEntries(members.map((m) => [m, true]));
    shared = Object.fromEntries(params.map((p) => [p.id, group ? group.params.includes(p.id) : p.defaultOn]));
    // Copying makes sense when creating a link; when editing, members already match
    copy = !group;
    editing = true;
  }

  const chosenList = $derived(others.filter((i) => chosen[i]));
  const sharedList = $derived(params.filter((p) => shared[p.id]).map((p) => p.id));

  async function save() {
    saving = true;
    await ctl.linkChannels(ch, chosenList, sharedList, copy);
    saving = false;
    editing = false;
  }

  /** Another link the channel is in, which it would leave */
  const otherLink = (index: number) => {
    const g = ctl.linkGroup({ kind: ch.kind, index });
    return g && !g.channels.includes(ch.index) ? g : undefined;
  };
</script>

<section class="panel link" class:linked={!!group}>
  {#if !editing}
    <div class="summary">
      <svg class="icon" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
        <path d="M6.5 9.5l3-3M7 4.5l1-1a2.8 2.8 0 0 1 4 4l-1 1M9 11.5l-1 1a2.8 2.8 0 0 1-4-4l1-1" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
      </svg>
      {#if group}
        <span class="label">Linked with</span>
        {#each members as m, i (m)}
          <a href="#/{ch.kind}/{m}"><ChannelLabel ch={{ kind: ch.kind, index: m }} /></a>{#if i < members.length - 1}<span class="sep">+</span>{/if}
        {/each}
        <span class="shares">shares {params.filter((p) => group.params.includes(p.id)).map((p) => p.label).join(', ')}</span>
        <span class="spacer"></span>
        <button onclick={startEdit}>Edit link</button>
        <button class="ghost" onclick={() => ctl.unlink(ch)}>Unlink</button>
      {:else}
        <span class="label">Not linked</span>
        <span class="shares">Link with other {ch.kind === 'in' ? 'inputs' : 'outputs'} so they share settings, e.g. left and right woofer.</span>
        <span class="spacer"></span>
        <button onclick={startEdit}>Link…</button>
      {/if}
    </div>
  {:else}
    <div class="editor">
      <fieldset>
        <legend class="label">Link with</legend>
        {#each others as i (i)}
          {@const other = otherLink(i)}
          <label class="check">
            <input type="checkbox" bind:checked={chosen[i]} />
            <ChannelLabel ch={{ kind: ch.kind, index: i }} />
            {#if other && chosen[i]}<span class="note">leaves its current link</span>{/if}
          </label>
        {/each}
      </fieldset>
      <fieldset>
        <legend class="label">Shared settings</legend>
        {#each params as p (p.id)}
          <label class="check"><input type="checkbox" bind:checked={shared[p.id]} />{p.label}</label>
        {/each}
      </fieldset>
      <div class="foot">
        <label class="check">
          <input type="checkbox" bind:checked={copy} />
          Copy this channel's shared settings to the others now
        </label>
        <span class="spacer"></span>
        <button class="ghost" onclick={() => (editing = false)}>Cancel</button>
        <button class="on" disabled={saving || chosenList.length === 0 || sharedList.length === 0} onclick={save}>
          {group ? 'Save link' : 'Link'}
        </button>
      </div>
      {#if copy}
        <p class="note">Only settings this app knows are copied; unknown ones are left as they are.</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .link {
    padding: 8px 12px;
  }
  .link.linked {
    border-left: 2px solid var(--accent);
  }
  .summary {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    min-height: 26px;
  }
  .icon {
    color: var(--muted);
    flex: none;
  }
  .linked .icon {
    color: var(--accent);
  }
  .summary a {
    color: var(--text);
    text-decoration: none;
    display: inline-flex;
  }
  .summary a:hover {
    color: var(--accent);
  }
  .sep {
    color: var(--dim);
  }
  .shares,
  .note {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--dim);
  }
  .spacer {
    flex: 1;
  }
  .editor {
    display: grid;
    gap: 10px;
  }
  fieldset {
    border: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
  }
  legend {
    width: 100%;
    padding: 0 0 4px;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
  }
  .check input {
    margin: 0;
    accent-color: var(--accent);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .note {
    margin: 0;
  }
</style>
