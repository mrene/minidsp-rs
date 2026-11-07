export interface Toast {
  id: number;
  message: string;
  kind: 'error' | 'info';
}

let nextId = 1;

export const toasts = $state<Toast[]>([]);

function push(message: string, kind: Toast['kind']) {
  const id = nextId++;
  toasts.push({ id, message, kind });
  setTimeout(() => dismiss(id), kind === 'error' ? 7000 : 4000);
}

export function reportError(error: unknown) {
  push(error instanceof Error ? error.message : String(error), 'error');
}

export function notify(message: string) {
  push(message, 'info');
}

export function dismiss(id: number) {
  const index = toasts.findIndex((t) => t.id === id);
  if (index >= 0) toasts.splice(index, 1);
}
