<script lang="ts">
  export interface Trace {
    id: string;
    color: string;
    values: number[];
    width?: number;
    dashed?: boolean;
    opacity?: number;
  }

  export interface Handle {
    id: number;
    freq: number;
    gain: number;
    color: string;
    label: string;
    active?: boolean;
    /** Whether dragging vertically changes the gain */
    movableGain: boolean;
  }

  interface Props {
    freqs: number[];
    traces: Trace[];
    handles?: Handle[];
    yMin?: number;
    yMax?: number;
    height?: number;
    ondrag?: (id: number, freq: number, gain: number) => void;
    /** Called with a multiplier for Q when scrolling over a handle */
    onq?: (id: number, factor: number) => void;
    onselect?: (id: number) => void;
  }

  let {
    freqs,
    traces,
    handles = [],
    yMin = -30,
    yMax = 15,
    height = 280,
    ondrag,
    onq,
    onselect,
  }: Props = $props();

  let width = $state(600);
  const pad = { l: 40, r: 10, t: 10, b: 22 };
  const clipId = `plot-clip-${Math.random().toString(36).slice(2)}`;

  const fMin = $derived(freqs[0]);
  const fMax = $derived(freqs[freqs.length - 1]);
  const plotW = $derived(Math.max(10, width - pad.l - pad.r));
  const plotH = $derived(height - pad.t - pad.b);

  const x = (f: number) => pad.l + (Math.log(f / fMin) / Math.log(fMax / fMin)) * plotW;
  const y = (db: number) => pad.t + ((yMax - db) / (yMax - yMin)) * plotH;
  const fromX = (px: number) => fMin * Math.pow(fMax / fMin, (px - pad.l) / plotW);
  const fromY = (py: number) => yMax - ((py - pad.t) / plotH) * (yMax - yMin);

  function path(values: number[]) {
    let d = '';
    for (let i = 0; i < values.length && i < freqs.length; i++) {
      // Keep far-off values bounded so the clip path does the trimming
      const v = Math.max(yMin - 40, Math.min(yMax + 40, values[i]));
      d += `${i ? 'L' : 'M'}${x(freqs[i]).toFixed(1)},${y(v).toFixed(1)}`;
    }
    return d;
  }

  const MAJOR = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
  const MINOR = [10, 30, 40, 60, 70, 80, 90, 300, 400, 600, 700, 800, 900, 3000, 4000, 6000, 7000, 8000, 9000];
  const fmtF = (f: number) => (f >= 1000 ? `${f / 1000}k` : `${f}`);

  const dbLines = $derived.by(() => {
    const step = yMax - yMin > 60 ? 12 : 6;
    const lines: number[] = [];
    for (let v = Math.ceil(yMin / step) * step; v <= yMax; v += step) lines.push(v);
    return lines;
  });

  // ---- Dragging handles ----

  let svg: SVGSVGElement | undefined = $state();
  let dragging: { id: number; movableGain: boolean } | undefined;

  function pointerPosition(event: PointerEvent | WheelEvent) {
    const rect = svg!.getBoundingClientRect();
    return { px: event.clientX - rect.left, py: event.clientY - rect.top };
  }

  function startDrag(event: PointerEvent, handle: Handle) {
    event.preventDefault();
    dragging = { id: handle.id, movableGain: handle.movableGain };
    svg?.setPointerCapture(event.pointerId);
    onselect?.(handle.id);
  }

  function moveDrag(event: PointerEvent) {
    if (!dragging || !ondrag) return;
    const { px, py } = pointerPosition(event);
    const freq = Math.min(fMax, Math.max(fMin, fromX(px)));
    const gain = Math.min(yMax, Math.max(yMin, fromY(py)));
    ondrag(dragging.id, freq, gain);
  }

  function endDrag(event: PointerEvent) {
    if (dragging) svg?.releasePointerCapture(event.pointerId);
    dragging = undefined;
  }

  function wheel(event: WheelEvent, handle: Handle) {
    if (!onq) return;
    event.preventDefault();
    onq(handle.id, event.deltaY < 0 ? 1.1 : 1 / 1.1);
  }

  function keydown(event: KeyboardEvent, handle: Handle) {
    if (!ondrag) return;
    const ratio = event.shiftKey ? 1.01 : 1.059; // ~1/12 octave, or fine
    const step = event.shiftKey ? 0.1 : 0.5;
    let { freq, gain } = handle;
    if (event.key === 'ArrowLeft') freq /= ratio;
    else if (event.key === 'ArrowRight') freq *= ratio;
    else if (event.key === 'ArrowUp' && handle.movableGain) gain += step;
    else if (event.key === 'ArrowDown' && handle.movableGain) gain -= step;
    else return;
    event.preventDefault();
    ondrag(handle.id, Math.min(fMax, Math.max(fMin, freq)), Math.min(yMax, Math.max(yMin, gain)));
  }
</script>

<div class="plot" bind:clientWidth={width}>
  <svg
    bind:this={svg}
    {width}
    {height}
    role="img"
    aria-label="Frequency response"
    onpointermove={moveDrag}
    onpointerup={endDrag}
    onpointercancel={endDrag}
  >
    <defs>
      <clipPath id={clipId}>
        <rect x={pad.l} y={pad.t} width={plotW} height={plotH} />
      </clipPath>
    </defs>

    <rect class="plot-bg" x={pad.l} y={pad.t} width={plotW} height={plotH} />

    <!-- Grid -->
    {#each MINOR as f (f)}
      {#if f >= fMin && f <= fMax}
        <line class="grid" x1={x(f)} x2={x(f)} y1={pad.t} y2={pad.t + plotH} />
      {/if}
    {/each}
    {#each MAJOR as f (f)}
      {#if f >= fMin && f <= fMax}
        <line class="grid major" x1={x(f)} x2={x(f)} y1={pad.t} y2={pad.t + plotH} />
        <text class="axis" x={x(f)} y={height - 6} text-anchor="middle">{fmtF(f)}</text>
      {/if}
    {/each}
    {#each dbLines as v (v)}
      <line class="grid" class:zero={v === 0} x1={pad.l} x2={pad.l + plotW} y1={y(v)} y2={y(v)} />
      <text class="axis" x={pad.l - 6} y={y(v) + 3.5} text-anchor="end">{v > 0 ? `+${v}` : v}</text>
    {/each}
    <rect class="frame" x={pad.l} y={pad.t} width={plotW} height={plotH} />

    <!-- Traces -->
    <g clip-path="url(#{clipId})">
      {#each traces as trace (trace.id)}
        <path
          d={path(trace.values)}
          fill="none"
          stroke={trace.color}
          stroke-width={trace.width ?? 1.5}
          stroke-opacity={trace.opacity ?? 1}
          stroke-dasharray={trace.dashed ? '4 3' : undefined}
          stroke-linejoin="round"
        />
      {/each}
    </g>

    <!-- Handles -->
    {#each handles as handle (handle.id)}
      {@const hx = x(handle.freq)}
      {@const hy = y(Math.max(yMin, Math.min(yMax, handle.gain)))}
      <g
        class="handle"
        class:active={handle.active}
        style:color={handle.color}
        transform="translate({hx} {hy})"
        role="slider"
        tabindex="0"
        aria-label="Band {handle.label}: {Math.round(handle.freq)} Hz, {handle.gain.toFixed(1)} dB"
        aria-valuenow={Math.round(handle.freq)}
        onpointerdown={(e) => startDrag(e, handle)}
        onwheel={(e) => wheel(e, handle)}
        onkeydown={(e) => keydown(e, handle)}
        onfocus={() => onselect?.(handle.id)}
      >
        <circle r="11" fill="transparent" />
        <circle r="7" fill="var(--bg)" stroke={handle.color} stroke-width="1.5" />
        <text y="3.5" text-anchor="middle" fill={handle.color}>{handle.label}</text>
      </g>
    {/each}
  </svg>
</div>

<style>
  .plot {
    width: 100%;
    user-select: none;
  }
  svg {
    display: block;
    touch-action: none;
  }
  .grid {
    stroke: var(--grid);
    stroke-width: 1;
  }
  .grid.major {
    stroke: var(--grid-major);
  }
  .grid.zero {
    stroke: var(--line-strong);
  }
  .plot-bg {
    fill: var(--raised);
  }
  .frame {
    fill: none;
    stroke: var(--line);
  }
  .axis {
    font-family: var(--mono);
    font-size: 9.5px;
    fill: var(--dim);
  }
  .handle {
    cursor: grab;
    outline: none;
  }
  .handle text {
    font-family: var(--mono);
    font-size: 8.5px;
    font-weight: 600;
    pointer-events: none;
  }
  .handle.active circle:nth-child(2),
  .handle:focus-visible circle:nth-child(2) {
    fill: currentColor;
    stroke-width: 2;
  }
  .handle.active text,
  .handle:focus-visible text {
    fill: var(--bg);
  }
</style>
