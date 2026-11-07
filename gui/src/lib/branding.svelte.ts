// Name, logo and accent color of the interface, stored by the daemon and shared by every client
import { api } from './api';
import type { Branding } from './types';

export const DEFAULT_TITLE = 'minidsp';
/** Accent of the built-in theme (see app.css) */
export const DEFAULT_ACCENT = '#0a8aa3';
/** Largest logo file accepted (the daemon also checks) */
export const MAX_LOGO_BYTES = 500 * 1024;

/** WCAG relative luminance of a #rrggbb color */
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Contrast ratio between two #rrggbb colors */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

class BrandingStore {
  /** What the daemon has */
  saved = $state<Branding>({ title: '' });
  loaded = $state(false);

  get title() {
    return this.saved.title || DEFAULT_TITLE;
  }

  async load() {
    try {
      this.saved = await api.branding();
    } catch {
      // Branding is optional; keep the defaults
    } finally {
      this.loaded = true;
      this.apply(this.saved);
    }
  }

  async save(branding: Branding) {
    this.saved = await api.setBranding(branding);
    this.apply(this.saved);
  }

  /** Applies branding to the page (also used for live previews while editing) */
  apply(branding: Branding) {
    if (typeof document === 'undefined') return;
    document.title = branding.title || DEFAULT_TITLE;

    const root = document.documentElement.style;
    if (branding.accent) {
      const [r, g, b] = [1, 3, 5].map((i) => parseInt(branding.accent!.slice(i, i + 2), 16));
      root.setProperty('--accent', branding.accent);
      root.setProperty('--accent-soft', `rgb(${r} ${g} ${b} / 0.12)`);
    } else {
      root.removeProperty('--accent');
      root.removeProperty('--accent-soft');
    }

    const icon = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
    if (icon) {
      icon.href = branding.logo ?? '/favicon.svg';
      icon.type = branding.logo ? branding.logo.slice(5, branding.logo.indexOf(';')) : 'image/svg+xml';
    }
  }
}

export const branding = new BrandingStore();
