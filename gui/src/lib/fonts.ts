// Font pairs the interface can use. Each is bundled into the build (served by minidspd, no
// internet needed) and only downloaded by a browser when it is selected.

export type FontId = 'system' | 'plex' | 'inter' | 'barlow' | 'space';

export interface FontPair {
  id: FontId;
  label: string;
  description: string;
  /** Text font stack */
  sans: string;
  /** Font for numbers and labels */
  mono: string;
  /** Loads the font files (latin subset, the weights the interface uses) */
  load: () => Promise<unknown>;
}

const SYSTEM_SANS = "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif";
const SYSTEM_MONO = "ui-monospace, 'SF Mono', SFMono-Regular, Menlo, Consolas, monospace";

export const FONT_PAIRS: FontPair[] = [
  {
    id: 'system',
    label: 'System',
    description: "Your device's own fonts (SF Pro and SF Mono on a Mac)",
    sans: SYSTEM_SANS,
    mono: SYSTEM_MONO,
    load: async () => {},
  },
  {
    id: 'plex',
    label: 'IBM Plex',
    description: 'IBM Plex Sans and Plex Mono: technical, engineered, like test equipment',
    sans: `'IBM Plex Sans', ${SYSTEM_SANS}`,
    mono: `'IBM Plex Mono', ${SYSTEM_MONO}`,
    load: () =>
      Promise.all([
        import('@fontsource/ibm-plex-sans/latin-400.css'),
        import('@fontsource/ibm-plex-sans/latin-500.css'),
        import('@fontsource/ibm-plex-sans/latin-600.css'),
        import('@fontsource/ibm-plex-mono/latin-400.css'),
        import('@fontsource/ibm-plex-mono/latin-500.css'),
        import('@fontsource/ibm-plex-mono/latin-600.css'),
      ]),
  },
  {
    id: 'inter',
    label: 'Inter + JetBrains Mono',
    description: 'Clean and neutral, with very legible numbers',
    sans: `'Inter', ${SYSTEM_SANS}`,
    mono: `'JetBrains Mono', ${SYSTEM_MONO}`,
    load: () =>
      Promise.all([
        import('@fontsource/inter/latin-400.css'),
        import('@fontsource/inter/latin-500.css'),
        import('@fontsource/inter/latin-600.css'),
        import('@fontsource/jetbrains-mono/latin-400.css'),
        import('@fontsource/jetbrains-mono/latin-500.css'),
        import('@fontsource/jetbrains-mono/latin-600.css'),
      ]),
  },
  {
    id: 'barlow',
    label: 'Barlow',
    description: 'Barlow and Barlow Semi Condensed: compact and industrial, like labels on audio gear',
    sans: `'Barlow', ${SYSTEM_SANS}`,
    mono: `'Barlow Semi Condensed', ${SYSTEM_MONO}`,
    load: () =>
      Promise.all([
        import('@fontsource/barlow/latin-400.css'),
        import('@fontsource/barlow/latin-500.css'),
        import('@fontsource/barlow/latin-600.css'),
        import('@fontsource/barlow-semi-condensed/latin-400.css'),
        import('@fontsource/barlow-semi-condensed/latin-500.css'),
        import('@fontsource/barlow-semi-condensed/latin-600.css'),
      ]),
  },
  {
    id: 'space',
    label: 'Space Grotesk + Space Mono',
    description: 'Distinctive and slightly retro-technical',
    sans: `'Space Grotesk', ${SYSTEM_SANS}`,
    mono: `'Space Mono', ${SYSTEM_MONO}`,
    load: () =>
      Promise.all([
        import('@fontsource/space-grotesk/latin-400.css'),
        import('@fontsource/space-grotesk/latin-500.css'),
        import('@fontsource/space-grotesk/latin-600.css'),
        // Space Mono only comes in regular and bold
        import('@fontsource/space-mono/latin-400.css'),
        import('@fontsource/space-mono/latin-700.css'),
      ]),
  },
];

export const fontPair = (id: string | undefined): FontPair =>
  FONT_PAIRS.find((f) => f.id === id) ?? FONT_PAIRS[0];
