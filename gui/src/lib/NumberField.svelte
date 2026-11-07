<script lang="ts">
  interface Props {
    value: number | undefined;
    min?: number;
    max?: number;
    step?: number;
    digits?: number;
    unit?: string;
    label: string;
    /** Hide the label visually (it is still announced) */
    hideLabel?: boolean;
    width?: string;
    disabled?: boolean;
    onset: (value: number) => void;
  }

  let {
    value,
    min = -Infinity,
    max = Infinity,
    step = 1,
    digits = 1,
    unit,
    label,
    hideLabel = false,
    width = '5.5em',
    disabled = false,
    onset,
  }: Props = $props();

  const id = `nf-${Math.random().toString(36).slice(2)}`;

  function commit(event: Event & { currentTarget: HTMLInputElement }) {
    const v = event.currentTarget.valueAsNumber;
    if (Number.isNaN(v)) {
      // Restore the current value
      event.currentTarget.value = value === undefined ? '' : value.toFixed(digits);
      return;
    }
    const clamped = Math.min(max, Math.max(min, v));
    event.currentTarget.value = clamped.toFixed(digits);
    if (clamped !== value) onset(clamped);
  }
</script>

<span class="field">
  <label for={id} class:sr-only={hideLabel} class="label">{label}</label>
  <span class="input">
    <input
      {id}
      type="number"
      {min}
      {max}
      {step}
      {disabled}
      style:width
      placeholder="—"
      value={value === undefined ? '' : value.toFixed(digits)}
      onchange={commit}
      onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
    />
    {#if unit}<span class="unit">{unit}</span>{/if}
  </span>
</span>

<style>
  .field {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .input {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .unit {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--dim);
  }
  input {
    text-align: right;
  }
</style>
