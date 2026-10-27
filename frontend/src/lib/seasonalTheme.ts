// plugged.host: seasonal themes over Theme v2 (Admin › Settings › General › Metadata, docs/features/theme.md#seasonal-themes)
import type { MantineColorsTuple } from '@mantine/core';
import { type AccentPreset, accentSwatch } from '@/lib/accent.ts';

export type SeasonalTheme = 'default' | 'halloween' | 'christmas';

/*
 * A season keeps Theme v2's layout and type and changes three things: the accent (one of the theme
 * colour's presets, so lib/accent.ts keeps its contrast guards), a tint on the obsidian surfaces,
 * and the decorations in the sidebar (elements/navigation/SeasonalDecor.tsx). Default is Theme v2
 * with the theme colour; choosing it removes everything here.
 */
interface Season {
  /** The accent, over the theme colour. */
  accent: AccentPreset;
  /** Theme v2's surface and text tokens (app.css :root), tinted. */
  tokens: Record<string, string>;
  /** Mantine's dark ramp (theme.ts), tinted the same way: text, nav, muted, faint, then surfaces. */
  dark: MantineColorsTuple;
}

const SEASONS: Record<Exclude<SeasonalTheme, 'default'>, Season> = {
  // pumpkin orange on a night-violet obsidian
  halloween: {
    accent: 'orange',
    tokens: {
      '--theme-bg-top': '#141019',
      '--theme-bg-bottom': '#0c0a0f',
      '--theme-sidebar': '#1e1a25',
      '--theme-sunken': 'rgb(17 13 23 / 0.5)',
      '--theme-dropdown': '#1a1720',
      '--theme-modal': '#1c1923',
      '--theme-raise': 'rgb(233 213 255 / 0.05)',
      '--theme-raise-2': 'rgb(233 213 255 / 0.1)',
      '--theme-raise-3': 'rgb(233 213 255 / 0.15)',
      '--theme-hairline': 'rgb(233 213 255 / 0.06)',
      '--theme-text-muted': '#a59fb2',
      '--theme-text-faint': '#78717f',
    },
    dark: [
      '#f3f4f6',
      '#d1d5db',
      '#a59fb2',
      '#78717f',
      '#2f2a37',
      '#29242f',
      '#1e1a25',
      '#1c1923',
      '#141019',
      '#0c0a0f',
    ],
  },
  // holly red on an evergreen obsidian
  christmas: {
    accent: 'red',
    tokens: {
      '--theme-bg-top': '#0f1513',
      '--theme-bg-bottom': '#0a0e0c',
      '--theme-sidebar': '#19201d',
      '--theme-sunken': 'rgb(12 18 15 / 0.5)',
      '--theme-dropdown': '#161c19',
      '--theme-modal': '#18201c',
      '--theme-raise': 'rgb(220 252 231 / 0.05)',
      '--theme-raise-2': 'rgb(220 252 231 / 0.1)',
      '--theme-raise-3': 'rgb(220 252 231 / 0.15)',
      '--theme-hairline': 'rgb(220 252 231 / 0.06)',
      '--theme-text-muted': '#9fa8a3',
      '--theme-text-faint': '#707a75',
    },
    dark: [
      '#f3f4f6',
      '#d1d5db',
      '#9fa8a3',
      '#707a75',
      '#2a322e',
      '#242c28',
      '#19201d',
      '#18201c',
      '#0f1513',
      '#0a0e0c',
    ],
  },
};

/** The season's look, or null for Default (and anything this build does not know). */
export function seasonFor(theme: SeasonalTheme | null | undefined): Season | null {
  return theme && theme !== 'default' ? (SEASONS[theme] ?? null) : null;
}

/** The colour the accent is made from: the season's, else the theme colour. */
export function seasonalAccentColor(theme: SeasonalTheme | null | undefined, themeColor: string | null | undefined) {
  const season = seasonFor(theme);

  return season ? accentSwatch(season.accent) : themeColor;
}
