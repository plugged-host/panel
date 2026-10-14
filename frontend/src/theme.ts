import { Badge, Button, MantineColorsTuple, MantineThemeOverride, ThemeIcon } from '@mantine/core';

/*
 * plugged.host Theme v2 on Mantine. The values are the ones the Pelican panel's theme uses
 * (plugged-host-theme-v2/resources/css/theme.css on the 2.2 branch of the plugins); app.css
 * carries the same --phv2-* tokens, the dark colour-scheme variables and the parts Mantine's
 * theme object does not reach.
 *
 * The accent is two-tier, as in Theme v2: blue-700 (#1d4ed8) fills, blue-500 (#3b82f6) is the
 * same accent as text or a ring on a dark surface, where blue-700 would be unreadable.
 */

// Tailwind blue, shifted one rung darker from 200 up so the filled shade (6) is blue-700.
const blue: MantineColorsTuple = [
  '#eff6ff',
  '#dbeafe',
  '#93c5fd',
  '#60a5fa',
  '#3b82f6',
  '#2563eb',
  '#1d4ed8',
  '#1e40af',
  '#1e3a8a',
  '#172554',
];

// Mantine's dark ramp mapped onto Theme v2: text, nav text, muted, faint, then the surfaces
// (borders, hover, sidebar, modals, page floor).
const dark: MantineColorsTuple = [
  '#f3f4f6',
  '#d1d5db',
  '#9ca3af',
  '#71717a',
  '#2c2e34',
  '#26282e',
  '#1c1e23',
  '#1a1c20',
  '#101319',
  '#0b0d10',
];

const sans = "'Space Grotesk', ui-sans-serif, system-ui, sans-serif";

export const pluggedHostTheme: MantineThemeOverride = {
  primaryColor: 'blue',
  primaryShade: 6,
  colors: { blue, dark },
  fontFamily: sans,
  fontFamilyMonospace: "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace",
  // Page titles: "text-xl font-semibold".
  headings: { fontFamily: sans, fontWeight: '600', sizes: { h1: { fontSize: '1.25rem', lineHeight: '1.75rem' } } },
  defaultRadius: 'md',
  cursorType: 'pointer',
  components: {
    // Theme v2 buttons are pills ("rounded-full"); badges are rounded-lg.
    Button: Button.extend({ defaultProps: { radius: 'xl' } }),
    Badge: Badge.extend({ defaultProps: { radius: 'md' } }),
    // Icon tiles (stat cards) stay flat: a tint, not a second filled accent next to the buttons.
    ThemeIcon: ThemeIcon.extend({ defaultProps: { variant: 'light' } }),
  },
};
