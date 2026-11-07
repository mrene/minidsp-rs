<script lang="ts">
  import { branding, contrast, DEFAULT_ACCENT, DEFAULT_TITLE, MAX_LOGO_BYTES } from './branding.svelte';
  import { notify, reportError } from './toasts.svelte';
  import type { Branding } from './types';

  // Edits preview live; Save stores them, Revert goes back to what's saved
  let draft = $state<Branding>({ ...branding.saved });
  let ready = $state(branding.loaded);
  let saving = $state(false);

  // Opening this page directly: start from the saved branding once it has loaded
  $effect(() => {
    if (!ready && branding.loaded) {
      draft = { ...branding.saved };
      ready = true;
    }
  });
  let fileInput: HTMLInputElement | undefined = $state();

  const changed = $derived(JSON.stringify(draft) !== JSON.stringify(branding.saved));
  const accent = $derived(draft.accent ?? DEFAULT_ACCENT);
  // The accent is used for text and controls on the light panels
  const readable = $derived(contrast(accent, '#f6f7f8') >= 3);

  $effect(() => {
    const preview = $state.snapshot(draft) as Branding;
    if (ready) branding.apply(preview);
  });

  // Leaving the page without saving drops the preview
  $effect(() => () => branding.apply(branding.saved));

  async function pickLogo(event: Event & { currentTarget: HTMLInputElement }) {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = '';
    if (!file) return;
    if (!['image/png', 'image/jpeg', 'image/webp', 'image/svg+xml'].includes(file.type)) {
      reportError(new Error(`${file.name} isn't a PNG, JPEG, WebP or SVG image`));
      return;
    }
    if (file.size > MAX_LOGO_BYTES) {
      reportError(new Error(`${file.name} is ${Math.round(file.size / 1024)} KB; use an image under 500 KB`));
      return;
    }
    draft.logo = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(file);
    });
  }

  async function save() {
    saving = true;
    try {
      await branding.save($state.snapshot(draft) as Branding);
      draft = { ...branding.saved };
      notify('Branding saved');
    } catch (e) {
      reportError(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="page">
  <section class="panel">
    <div class="panel-head">
      <h2>Branding</h2>
      <div class="buttons">
        <button class="ghost" disabled={!changed || saving} onclick={() => (draft = { ...branding.saved })}>Revert</button>
        <button class="on" disabled={!changed || saving} onclick={save}>Save</button>
      </div>
    </div>
    <div class="panel-body form">
      <label class="label" for="brand-title">Name</label>
      <div>
        <input id="brand-title" type="text" maxlength="40" placeholder={DEFAULT_TITLE} bind:value={draft.title} />
        <p class="help">Shown in the header and the browser tab.</p>
      </div>

      <span class="label">Logo</span>
      <div class="logo-row">
        <div class="logo-preview">
          {#if draft.logo}<img src={draft.logo} alt="Logo preview" />{:else}<span class="help">Default</span>{/if}
        </div>
        <input bind:this={fileInput} type="file" accept="image/png,image/jpeg,image/webp,image/svg+xml" class="sr-only" tabindex="-1" onchange={pickLogo} />
        <button onclick={() => fileInput?.click()}>Choose image…</button>
        {#if draft.logo}<button class="ghost" onclick={() => (draft.logo = undefined)}>Remove</button>{/if}
        <p class="help full">PNG, JPEG, WebP or SVG, under 500 KB. Also used as the browser-tab icon. A square image works best.</p>
      </div>

      <label class="label" for="brand-accent">Accent</label>
      <div>
        <div class="accent-row">
          <input id="brand-accent" type="color" value={accent} oninput={(e) => (draft.accent = e.currentTarget.value)} />
          <input type="text" class="num" aria-label="Accent color hex value" value={accent} maxlength="7"
            onchange={(e) => {
              const v = e.currentTarget.value.trim();
              if (/^#[0-9a-fA-F]{6}$/.test(v)) draft.accent = v.toLowerCase();
              else e.currentTarget.value = accent;
            }} />
          {#if draft.accent}<button class="ghost" onclick={() => (draft.accent = undefined)}>Default</button>{/if}
          <span class="swatch-demo on">Selected</span>
        </div>
        {#if !readable}
          <p class="warning">This color is hard to read on the light background (contrast {contrast(accent, '#f6f7f8').toFixed(1)}:1, at least 3:1 is recommended). Pick a darker shade.</p>
        {:else}
          <p class="help">Used for selected controls, sliders and links.</p>
        {/if}
      </div>
    </div>
  </section>
</div>

<style>
  .page {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
    max-width: 760px;
  }
  .buttons {
    display: flex;
    gap: 6px;
  }
  .form {
    display: grid;
    grid-template-columns: 6rem minmax(0, 1fr);
    gap: 16px 14px;
    align-items: start;
  }
  .form > .label {
    padding-top: 6px;
  }
  #brand-title {
    width: 100%;
    max-width: 22em;
    font-family: var(--sans);
    font-size: 13px;
  }
  .help,
  .warning {
    margin: 4px 0 0;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--dim);
  }
  .warning {
    color: var(--warn);
  }
  .logo-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .full {
    flex-basis: 100%;
  }
  .logo-preview {
    width: 56px;
    height: 56px;
    display: grid;
    place-items: center;
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .logo-preview img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .accent-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  input[type='color'] {
    width: 36px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    background: none;
  }
  .accent-row input[type='text'] {
    width: 6em;
  }
  .swatch-demo {
    font-family: var(--mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 4px 9px;
    border: 1px solid var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
    border-radius: var(--radius);
  }
  @media (max-width: 600px) {
    .form {
      grid-template-columns: minmax(0, 1fr);
      gap: 6px;
    }
    .form > div {
      margin-bottom: 10px;
    }
  }
</style>
