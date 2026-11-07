export interface Toast {
  id: number;
  message: string;
}

let nextId = 1;

export const toasts = $state<Toast[]>([]);

export function reportError(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  const id = nextId++;
  toasts.push({ id, message });
  setTimeout(() => dismiss(id), 6000);
}

export function dismiss(id: number) {
  const index = toasts.findIndex((t) => t.id === id);
  if (index >= 0) toasts.splice(index, 1);
}
