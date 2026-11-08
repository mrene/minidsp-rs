import type {
  Branding,
  Locks,
  ChannelLinks,
  ChannelNames,
  Config,
  Device,
  DeviceLayout,
  HistoryEntry,
  MasterStatus,
  Preset,
  PresetSummary,
  StatusSummary,
} from './types';

const presetUrl = (id: string) => `/presets/${encodeURIComponent(id)}`;

export class ApiError extends Error {}

async function request<T>(method: 'GET' | 'POST' | 'PUT' | 'DELETE', path: string, body?: unknown): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, {
      method,
      headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError('Cannot reach minidspd. Is the daemon running?');
  }

  const text = await res.text();
  if (!res.ok) {
    let message = text || `HTTP ${res.status}`;
    try {
      const parsed = JSON.parse(text);
      if (typeof parsed?.message === 'string') message = parsed.message;
    } catch {
      // Not JSON, keep the raw text
    }
    throw new ApiError(message);
  }
  return (text ? JSON.parse(text) : undefined) as T;
}

export const api = {
  devices: () => request<Device[]>('GET', '/devices'),
  layout: (device: number) => request<DeviceLayout>('GET', `/devices/${device}/layout`),
  status: (device: number) => request<StatusSummary>('GET', `/devices/${device}`),
  setMaster: (device: number, status: MasterStatus) =>
    request<MasterStatus>('POST', `/devices/${device}`, status),
  applyConfig: (device: number, config: Config) =>
    request<void>('POST', `/devices/${device}/config`, config),

  branding: () => request<Branding>('GET', '/branding'),
  setBranding: (branding: Branding) => request<Branding>('PUT', '/branding', branding),

  locks: (device: number) => request<Locks>('GET', `/devices/${device}/locks`),
  setLocks: (device: number, locks: Locks) => request<Locks>('PUT', `/devices/${device}/locks`, locks),
  links: (device: number) => request<ChannelLinks>('GET', `/devices/${device}/links`),
  setLinks: (device: number, links: ChannelLinks) => request<ChannelLinks>('PUT', `/devices/${device}/links`, links),
  names: (device: number) => request<ChannelNames>('GET', `/devices/${device}/names`),
  setNames: (device: number, names: ChannelNames) => request<ChannelNames>('PUT', `/devices/${device}/names`, names),

  presets: () => request<PresetSummary[]>('GET', '/presets'),
  deletedPresets: () => request<PresetSummary[]>('GET', '/deleted-presets'),
  preset: (id: string) => request<Preset>('GET', presetUrl(id)),
  /** Saves a new preset; `action` describes it in the history */
  createPreset: (preset: Preset, action?: string) => request<{ id: string }>('POST', '/presets', { ...preset, action }),
  updatePreset: (id: string, preset: Preset, action?: string) =>
    request<{ id: string }>('PUT', presetUrl(id), { ...preset, action }),
  renamePreset: (id: string, name: string) => request<{ id: string }>('POST', `${presetUrl(id)}/rename`, { name }),
  presetHistory: (id: string) => request<HistoryEntry[]>('GET', `${presetUrl(id)}/history`),
  undoPreset: (id: string) => request<HistoryEntry>('POST', `${presetUrl(id)}/undo`),
  rollbackPreset: (id: string, version: number) =>
    request<HistoryEntry>('POST', `${presetUrl(id)}/rollback`, { version }),
  deletePreset: (id: string) => request<{ id: string }>('DELETE', presetUrl(id)),
  restorePreset: (id: string) => request<HistoryEntry>('POST', `${presetUrl(id)}/restore`),
  /** Switches the device to `slot` and writes the preset into it */
  applyPreset: (device: number, id: string, slot: number) =>
    request<MasterStatus>('POST', `/devices/${device}/presets/${encodeURIComponent(id)}/apply`, { slot }),
};

export type ConnectionState = 'connecting' | 'live' | 'offline';

/**
 * Streams status updates and levels for a device, reconnecting automatically.
 * Returns a function that closes the stream.
 */
export function watchDevice(
  device: number,
  onMessage: (summary: StatusSummary) => void,
  onState: (state: ConnectionState) => void,
): () => void {
  let socket: WebSocket | undefined;
  let retryTimer: ReturnType<typeof setTimeout> | undefined;
  let retryDelay = 1000;
  let closed = false;

  const connect = () => {
    onState('connecting');
    const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
    socket = new WebSocket(`${scheme}://${location.host}/devices/${device}?levels&poll`);

    socket.onopen = () => {
      retryDelay = 1000;
      onState('live');
    };
    socket.onmessage = (event) => {
      try {
        onMessage(JSON.parse(event.data));
      } catch {
        // Ignore malformed frames
      }
    };
    socket.onclose = () => {
      if (closed) return;
      onState('offline');
      retryTimer = setTimeout(connect, retryDelay);
      retryDelay = Math.min(retryDelay * 2, 8000);
    };
  };

  connect();

  return () => {
    closed = true;
    clearTimeout(retryTimer);
    socket?.close();
  };
}

/**
 * Wraps an async setter so that at most one request is in flight at a time and only the most
 * recent value is sent next. Dragging a slider therefore never queues up stale values, and the
 * final position is always the last one applied.
 */
export function latestSender<T>(send: (value: T) => Promise<unknown>): (value: T) => void {
  let inFlight = false;
  let next: { value: T } | undefined;

  const run = async (value: T) => {
    inFlight = true;
    try {
      await send(value);
    } catch {
      // `send` is responsible for reporting its own errors
    } finally {
      inFlight = false;
      if (next) {
        const { value: queued } = next;
        next = undefined;
        void run(queued);
      }
    }
  };

  return (value: T) => {
    if (inFlight) next = { value };
    else void run(value);
  };
}

export function msToDuration(ms: number): { secs: number; nanos: number } {
  const secs = Math.floor(ms / 1000);
  const nanos = Math.round((ms - secs * 1000) * 1e6);
  return { secs, nanos };
}
