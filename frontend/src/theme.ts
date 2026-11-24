import { Badge, Button, MantineColorsTuple, MantineThemeOverride, Modal, Tabs, ThemeIcon } from '@mantine/core';

/*
 * plugged.host Theme v2 on Mantine. The layout values are the ones the Pelican panel's theme uses
 * (plugged-host-theme-v2/resources/css/theme.css on the 2.2 branch of the plugins); the colours
 * are the plugged.host brand kit's (Socket blue as the accent only, plain cool greys and the shared
 * status colours, docs/features/theme.md). app.css carries the same tokens as --theme-*, the dark
 * colour-scheme variables and the parts Mantine's theme object does not reach.
 *
 * The accent is two-tier: Socket blue (#3a5bff, white text 5.1:1) fills, blue-400 (#688cfc) is the
 * same accent as text or a ring on a dark surface. The theme colour (Admin › Settings › General ›
 * Metadata) swaps it for another one (lib/accent.ts, App.tsx).
 */

// Socket blue in Mantine's slots: 0-3 tints, 4 text and rings on dark (brand 400), 6 the fill
// (brand 500), 7 its hover (brand 600).
const blue: MantineColorsTuple = [
  '#eef3ff',
  '#dce6ff',
  '#bed0fe',
  '#95b2fd',
  '#688cfc',
  '#3a5bff',
  '#3a5bff',
  '#2449d8',
  '#012d97',
  '#001b4c',
];

// Mantine's dark ramp mapped onto Theme v2 with the brand kit's plain greys (never tinted toward
// the blue): text, nav text, muted, faint, then line, surface-2, surface, modals, page floor.
const dark: MantineColorsTuple = [
  '#f1f2f4',
  '#cbced1',
  '#a2a5a9',
  '#777b7f',
  '#414448',
  '#292c2f',
  '#181a1c',
  '#151719',
  '#111315',
  '#0b0c0e',
];

// The brand's status colours (success, warning, danger), shared with Zaleris and switched.cloud,
// each as a ramp with the base colour as the fill (6) and the design system's accessible text
// colour on dark as 4; app.css sets their light (badge) variants to the -soft backgrounds.
const green: MantineColorsTuple = [
  '#e3fcee',
  '#d4f6e2',
  '#bbebd0',
  '#96dbb6',
  '#1fb57a',
  '#1fb57a',
  '#1fb57a',
  '#0c9361',
  '#07764d',
  '#03452b',
];

const yellow: MantineColorsTuple = [
  '#fff5e9',
  '#ffeed9',
  '#ffe2bf',
  '#fdd199',
  '#f2a93b',
  '#f2a93b',
  '#f2a93b',
  '#c5861f',
  '#a06a0b',
  '#5c3a01',
];

const red: MantineColorsTuple = [
  '#fff0f1',
  '#fee2e4',
  '#fecacd',
  '#fea6ad',
  '#fd5973',
  '#ef4565',
  '#ef4565',
  '#c6304f',
  '#a51e3d',
  '#68011f',
];

const sans = "'Space Grotesk', ui-sans-serif, system-ui, sans-serif";

export const themeV2: MantineThemeOverride = {
  primaryColor: 'blue',
  primaryShade: 6,
  colors: { blue, dark, green, yellow, red },
  fontFamily: sans,
  // Filament's copy is "text-sm" almost everywhere: Mantine's default text size (md) follows it,
  // so Text, paragraphs and inputs agree with the 14px body (app.css).
  fontSizes: { md: '0.875rem' },
  fontFamilyMonospace: "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace",
  // Filament's type scale: page titles "text-xl font-semibold", section and modal headings
  // "text-base font-semibold", nothing bigger.
  headings: {
    fontFamily: sans,
    fontWeight: '600',
    sizes: {
      h1: { fontSize: '1.25rem', lineHeight: '1.75rem' },
      h2: { fontSize: '1.125rem', lineHeight: '1.75rem' },
      h3: { fontSize: '1.125rem', lineHeight: '1.75rem' },
      h4: { fontSize: '1rem', lineHeight: '1.5rem' },
      h5: { fontSize: '1rem', lineHeight: '1.5rem' },
      h6: { fontSize: '0.875rem', lineHeight: '1.25rem' },
    },
  },
  // The brand kit's radii: 6px small parts, 10px buttons and inputs, 16px cards.
  radius: { sm: '6px', md: '10px', lg: '16px' },
  defaultRadius: 'md',
  cursorType: 'pointer',
  components: {
    // Buttons have the brand kit's 10px corners; badges are rounded-lg and tinted, never solid
    // (Filament's "bg-custom-400/10 text-custom-400").
    Button: Button.extend({ defaultProps: { radius: 'md' } }),
    Badge: Badge.extend({ defaultProps: { radius: 'md', variant: 'light' } }),
    // Filament's modal overlay: "bg-gray-950/75", no blur.
    Modal: Modal.extend({ defaultProps: { overlayProps: { backgroundOpacity: 0.75, color: '#0b0c0e', blur: 0 } } }),
    // Underlined tabs (inside cards and modals): the bright accent as the line, like Filament's
    // contained tabs ("border-primary-500 text-primary-500").
    Tabs: Tabs.extend({ defaultProps: { color: 'blue.4' } }),
    // Icon tiles (stat cards) stay flat: a tint, not a second filled accent next to the buttons.
    ThemeIcon: ThemeIcon.extend({ defaultProps: { variant: 'light' } }),
  },
};
