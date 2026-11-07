// Hash-based navigation: #/overview, #/routing, #/in/0, #/out/3
import type { ChannelRef } from './controller.svelte';

export type Route =
  | { page: 'overview' }
  | { page: 'routing' }
  | { page: 'presets' }
  | { page: 'channel'; ch: ChannelRef };

function parse(hash: string): Route {
  const parts = hash.replace(/^#\/?/, '').split('/');
  if (parts[0] === 'routing') return { page: 'routing' };
  if (parts[0] === 'presets') return { page: 'presets' };
  if ((parts[0] === 'in' || parts[0] === 'out') && /^\d+$/.test(parts[1] ?? '')) {
    return { page: 'channel', ch: { kind: parts[0], index: Number(parts[1]) } };
  }
  return { page: 'overview' };
}

export const href = {
  overview: '#/overview',
  routing: '#/routing',
  presets: '#/presets',
  channel: (ch: ChannelRef) => `#/${ch.kind}/${ch.index}`,
};

class Router {
  route = $state<Route>(parse(typeof location === 'undefined' ? '' : location.hash));

  constructor() {
    if (typeof window !== 'undefined') {
      window.addEventListener('hashchange', () => (this.route = parse(location.hash)));
    }
  }

  isChannel(ch: ChannelRef) {
    return this.route.page === 'channel' && this.route.ch.kind === ch.kind && this.route.ch.index === ch.index;
  }
}

export const router = new Router();
