<script lang="ts">
  interface Props {
    label: string;
    /** Current value, or undefined if unknown */
    value: number | undefined;
    min: number;
    max: number;
    step?: number;
    unit?: string;
    digits?: number;
    disabled?: boolean;
    /** Where the slider rests when the value is unknown */
    restAt?: number;
    onset: (value: number) => void;
  }

  let {
    label,
    value,
    min,
    max,
    step = 0.5,
    unit = 'dB',
    digits = 1,
    disabled = false,
    restAt = 0,
    onset,
  }: Props = $props();

  // While dragging, show the local position instead of updates from the device
  let dragging = $state(false);
  let local = $state(0);
  const shown = $derived(dragging ? local : value);
  const position = $derived(Math.min(max, Math.max(min, shown ?? restAt)));

  const id = `fader-${Math.random().toString(36).slice(2)}`;

  function onInput(event: Event & { currentTarget: HTMLInputElement }) {
    dragging = true;
    local = Number(event.currentTarget.value);
    onset(local);
  }

  function onChange(event: Event & { currentTarget: HTMLInputElement }) {
    local = Number(event.currentTarget.value);
    onset(local);
    dragging = false;
  }
</script>

<div class="fader" class:unknown={shown === undefined}>
  <label for={id} class="label">{label}</label>
  <input
    {id}
    type="range"
    {min}
    {max}
    {step}
    {disabled}
    value={position}
    oninput={onInput}
    onchange={onChange}
    onpointerup={() => (dragging = false)}
  />
  <output for={id} class="num">
    {shown === undefined ? '—' : shown.toFixed(digits)}<span class="unit">{unit}</span>
  </output>
</div>

<style>
  .fader {
    display: grid;
    grid-template-columns: 4.2rem minmax(0, 1fr) 5.6rem;
    align-items: center;
    gap: 10px;
  }
  output {
    text-align: right;
    font-size: 12.5px;
  }
  .unit {
    margin-left: 4px;
    font-size: 10px;
    color: var(--dim);
  }
  .unknown output {
    color: var(--dim);
  }
  /* A slider resting at a default position shouldn't look like a real setting */
  .unknown input {
    opacity: 0.35;
  }
  input {
    width: 100%;
  }
</style>
