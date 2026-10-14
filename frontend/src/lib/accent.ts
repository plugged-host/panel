import type { MantineColorsTuple } from '@mantine/core';

/*
 * plugged.host: Theme v2's accent colour, the Pelican panel's (plugged-host-theme-v2
 * Support\Accent on the 2.2 branch of the plugins), set here by the panel's theme colour
 * (Admin › Settings › Metadata).
 *
 * theme.ts and app.css are written in blue: Mantine's blue palette and the --phv2-accent*
 * properties, plus Tailwind's blue utilities that upstream's components use. Any other accent
 * overrides exactly those (App.tsx); blue, the default, overrides nothing, so the panel stays what
 * the stylesheet says.
 *
 * A preset is one of Tailwind's palettes, mapped the way the theme maps blue: fill 700, hover 800,
 * text/ring 500, glow 400, the palette one rung darker than Tailwind's from 200 up. A custom colour
 * is the fill itself, with the rest of the scale generated around its hue and saturation
 * (Filament's palette). Two guards keep the fill/text split the theme depends on: the fill darkens
 * until white text on it reaches 4.5:1, and the text rung moves lighter until it reaches 4.5:1 on
 * the sidebar.
 */

type Shade = 50 | 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 950;
type Palette = Record<Shade, string>;
type Oklch = [number, number, number];

/** Tailwind's chromatic palettes (Filament's copies). The greys are left out: a grey accent disappears into the surfaces. */
const PALETTES = {
  blue: {
    50: 'oklch(0.97 0.014 254.604)',
    100: 'oklch(0.932 0.032 255.585)',
    200: 'oklch(0.882 0.059 254.128)',
    300: 'oklch(0.809 0.105 251.813)',
    400: 'oklch(0.707 0.165 254.624)',
    500: 'oklch(0.623 0.214 259.815)',
    600: 'oklch(0.546 0.245 262.881)',
    700: 'oklch(0.488 0.243 264.376)',
    800: 'oklch(0.424 0.199 265.638)',
    900: 'oklch(0.379 0.146 265.522)',
    950: 'oklch(0.282 0.091 267.935)',
  },
  indigo: {
    50: 'oklch(0.962 0.018 272.314)',
    100: 'oklch(0.93 0.034 272.788)',
    200: 'oklch(0.87 0.065 274.039)',
    300: 'oklch(0.785 0.115 274.713)',
    400: 'oklch(0.673 0.182 276.935)',
    500: 'oklch(0.585 0.233 277.117)',
    600: 'oklch(0.511 0.262 276.966)',
    700: 'oklch(0.457 0.24 277.023)',
    800: 'oklch(0.398 0.195 277.366)',
    900: 'oklch(0.359 0.144 278.697)',
    950: 'oklch(0.257 0.09 281.288)',
  },
  violet: {
    50: 'oklch(0.969 0.016 293.756)',
    100: 'oklch(0.943 0.029 294.588)',
    200: 'oklch(0.894 0.057 293.283)',
    300: 'oklch(0.811 0.111 293.571)',
    400: 'oklch(0.702 0.183 293.541)',
    500: 'oklch(0.606 0.25 292.717)',
    600: 'oklch(0.541 0.281 293.009)',
    700: 'oklch(0.491 0.27 292.581)',
    800: 'oklch(0.432 0.232 292.759)',
    900: 'oklch(0.38 0.189 293.745)',
    950: 'oklch(0.283 0.141 291.089)',
  },
  purple: {
    50: 'oklch(0.977 0.014 308.299)',
    100: 'oklch(0.946 0.033 307.174)',
    200: 'oklch(0.902 0.063 306.703)',
    300: 'oklch(0.827 0.119 306.383)',
    400: 'oklch(0.714 0.203 305.504)',
    500: 'oklch(0.627 0.265 303.9)',
    600: 'oklch(0.558 0.288 302.321)',
    700: 'oklch(0.496 0.265 301.924)',
    800: 'oklch(0.438 0.218 303.724)',
    900: 'oklch(0.381 0.176 304.987)',
    950: 'oklch(0.291 0.149 302.717)',
  },
  fuchsia: {
    50: 'oklch(0.977 0.017 320.058)',
    100: 'oklch(0.952 0.037 318.852)',
    200: 'oklch(0.903 0.076 319.62)',
    300: 'oklch(0.833 0.145 321.434)',
    400: 'oklch(0.74 0.238 322.16)',
    500: 'oklch(0.667 0.295 322.15)',
    600: 'oklch(0.591 0.293 322.896)',
    700: 'oklch(0.518 0.253 323.949)',
    800: 'oklch(0.452 0.211 324.591)',
    900: 'oklch(0.401 0.17 325.612)',
    950: 'oklch(0.293 0.136 325.661)',
  },
  pink: {
    50: 'oklch(0.971 0.014 343.198)',
    100: 'oklch(0.948 0.028 342.258)',
    200: 'oklch(0.899 0.061 343.231)',
    300: 'oklch(0.823 0.12 346.018)',
    400: 'oklch(0.718 0.202 349.761)',
    500: 'oklch(0.656 0.241 354.308)',
    600: 'oklch(0.592 0.249 0.584)',
    700: 'oklch(0.525 0.223 3.958)',
    800: 'oklch(0.459 0.187 3.815)',
    900: 'oklch(0.408 0.153 2.432)',
    950: 'oklch(0.284 0.109 3.907)',
  },
  rose: {
    50: 'oklch(0.969 0.015 12.422)',
    100: 'oklch(0.941 0.03 12.58)',
    200: 'oklch(0.892 0.058 10.001)',
    300: 'oklch(0.81 0.117 11.638)',
    400: 'oklch(0.712 0.194 13.428)',
    500: 'oklch(0.645 0.246 16.439)',
    600: 'oklch(0.586 0.253 17.585)',
    700: 'oklch(0.514 0.222 16.935)',
    800: 'oklch(0.455 0.188 13.697)',
    900: 'oklch(0.41 0.159 10.272)',
    950: 'oklch(0.271 0.105 12.094)',
  },
  red: {
    50: 'oklch(0.971 0.013 17.38)',
    100: 'oklch(0.936 0.032 17.717)',
    200: 'oklch(0.885 0.062 18.334)',
    300: 'oklch(0.808 0.114 19.571)',
    400: 'oklch(0.704 0.191 22.216)',
    500: 'oklch(0.637 0.237 25.331)',
    600: 'oklch(0.577 0.245 27.325)',
    700: 'oklch(0.505 0.213 27.518)',
    800: 'oklch(0.444 0.177 26.899)',
    900: 'oklch(0.396 0.141 25.723)',
    950: 'oklch(0.258 0.092 26.042)',
  },
  orange: {
    50: 'oklch(0.98 0.016 73.684)',
    100: 'oklch(0.954 0.038 75.164)',
    200: 'oklch(0.901 0.076 70.697)',
    300: 'oklch(0.837 0.128 66.29)',
    400: 'oklch(0.75 0.183 55.934)',
    500: 'oklch(0.705 0.213 47.604)',
    600: 'oklch(0.646 0.222 41.116)',
    700: 'oklch(0.553 0.195 38.402)',
    800: 'oklch(0.47 0.157 37.304)',
    900: 'oklch(0.408 0.123 38.172)',
    950: 'oklch(0.266 0.079 36.259)',
  },
  amber: {
    50: 'oklch(0.987 0.022 95.277)',
    100: 'oklch(0.962 0.059 95.617)',
    200: 'oklch(0.924 0.12 95.746)',
    300: 'oklch(0.879 0.169 91.605)',
    400: 'oklch(0.828 0.189 84.429)',
    500: 'oklch(0.769 0.188 70.08)',
    600: 'oklch(0.666 0.179 58.318)',
    700: 'oklch(0.555 0.163 48.998)',
    800: 'oklch(0.473 0.137 46.201)',
    900: 'oklch(0.414 0.112 45.904)',
    950: 'oklch(0.279 0.077 45.635)',
  },
  yellow: {
    50: 'oklch(0.987 0.026 102.212)',
    100: 'oklch(0.973 0.071 103.193)',
    200: 'oklch(0.945 0.129 101.54)',
    300: 'oklch(0.905 0.182 98.111)',
    400: 'oklch(0.852 0.199 91.936)',
    500: 'oklch(0.795 0.184 86.047)',
    600: 'oklch(0.681 0.162 75.834)',
    700: 'oklch(0.554 0.135 66.442)',
    800: 'oklch(0.476 0.114 61.907)',
    900: 'oklch(0.421 0.095 57.708)',
    950: 'oklch(0.286 0.066 53.813)',
  },
  lime: {
    50: 'oklch(0.986 0.031 120.757)',
    100: 'oklch(0.967 0.067 122.328)',
    200: 'oklch(0.938 0.127 124.321)',
    300: 'oklch(0.897 0.196 126.665)',
    400: 'oklch(0.841 0.238 128.85)',
    500: 'oklch(0.768 0.233 130.85)',
    600: 'oklch(0.648 0.2 131.684)',
    700: 'oklch(0.532 0.157 131.589)',
    800: 'oklch(0.453 0.124 130.933)',
    900: 'oklch(0.405 0.101 131.063)',
    950: 'oklch(0.274 0.072 132.109)',
  },
  green: {
    50: 'oklch(0.982 0.018 155.826)',
    100: 'oklch(0.962 0.044 156.743)',
    200: 'oklch(0.925 0.084 155.995)',
    300: 'oklch(0.871 0.15 154.449)',
    400: 'oklch(0.792 0.209 151.711)',
    500: 'oklch(0.723 0.219 149.579)',
    600: 'oklch(0.627 0.194 149.214)',
    700: 'oklch(0.527 0.154 150.069)',
    800: 'oklch(0.448 0.119 151.328)',
    900: 'oklch(0.393 0.095 152.535)',
    950: 'oklch(0.266 0.065 152.934)',
  },
  emerald: {
    50: 'oklch(0.979 0.021 166.113)',
    100: 'oklch(0.95 0.052 163.051)',
    200: 'oklch(0.905 0.093 164.15)',
    300: 'oklch(0.845 0.143 164.978)',
    400: 'oklch(0.765 0.177 163.223)',
    500: 'oklch(0.696 0.17 162.48)',
    600: 'oklch(0.596 0.145 163.225)',
    700: 'oklch(0.508 0.118 165.612)',
    800: 'oklch(0.432 0.095 166.913)',
    900: 'oklch(0.378 0.077 168.94)',
    950: 'oklch(0.262 0.051 172.552)',
  },
  teal: {
    50: 'oklch(0.984 0.014 180.72)',
    100: 'oklch(0.953 0.051 180.801)',
    200: 'oklch(0.91 0.096 180.426)',
    300: 'oklch(0.855 0.138 181.071)',
    400: 'oklch(0.777 0.152 181.912)',
    500: 'oklch(0.704 0.14 182.503)',
    600: 'oklch(0.6 0.118 184.704)',
    700: 'oklch(0.511 0.096 186.391)',
    800: 'oklch(0.437 0.078 188.216)',
    900: 'oklch(0.386 0.063 188.416)',
    950: 'oklch(0.277 0.046 192.524)',
  },
  cyan: {
    50: 'oklch(0.984 0.019 200.873)',
    100: 'oklch(0.956 0.045 203.388)',
    200: 'oklch(0.917 0.08 205.041)',
    300: 'oklch(0.865 0.127 207.078)',
    400: 'oklch(0.789 0.154 211.53)',
    500: 'oklch(0.715 0.143 215.221)',
    600: 'oklch(0.609 0.126 221.723)',
    700: 'oklch(0.52 0.105 223.128)',
    800: 'oklch(0.45 0.085 224.283)',
    900: 'oklch(0.398 0.07 227.392)',
    950: 'oklch(0.302 0.056 229.695)',
  },
  sky: {
    50: 'oklch(0.977 0.013 236.62)',
    100: 'oklch(0.951 0.026 236.824)',
    200: 'oklch(0.901 0.058 230.902)',
    300: 'oklch(0.828 0.111 230.318)',
    400: 'oklch(0.746 0.16 232.661)',
    500: 'oklch(0.685 0.169 237.323)',
    600: 'oklch(0.588 0.158 241.966)',
    700: 'oklch(0.5 0.134 242.749)',
    800: 'oklch(0.443 0.11 240.79)',
    900: 'oklch(0.391 0.09 240.876)',
    950: 'oklch(0.293 0.066 243.157)',
  },
} satisfies Record<string, Palette>;

export type AccentPreset = keyof typeof PALETTES;

export const ACCENT_PRESETS = Object.keys(PALETTES) as AccentPreset[];

/** Theme v2's own fill (blue-700), the default. */
export const DEFAULT_ACCENT = '#1d4ed8';

/** What accent text sits on most: the sidebar, --phv2-sidebar. */
const SURFACE = '#1c1e23';

const MIN_CONTRAST = 4.5;

/** Chroma of the 700 rung in Filament's generated palette, the rung a custom colour stands in for. */
const GENERATED_CHROMA_700 = 0.14940909090909;

/** Filament's generated palette: lightness and chroma per shade, around a colour's hue. */
const GENERATED: Record<Shade, [number, number]> = {
  50: [0.97717647058824, 0.01395454545455],
  100: [0.95035294117647, 0.03272727272727],
  200: [0.90547058823529, 0.06318181818182],
  300: [0.84047058823529, 0.10604545454546],
  400: [0.75352941176471, 0.15027272727273],
  500: [0.68270588235294, 0.17009090909091],
  600: [0.59782352941176, 0.16913636363636],
  700: [0.51494117647059, 0.14940909090909],
  800: [0.44611764705882, 0.12331818181818],
  900: [0.39458823529412, 0.09963636363636],
  950: [0.27788235294118, 0.07136363636364],
};

export interface Accent {
  /** The --phv2-accent* custom properties and Tailwind's blue scale (--color-blue-*), by name. */
  tokens: Record<string, string>;
  /** Mantine's primary (blue) palette. */
  palette: MantineColorsTuple;
}

const round3 = (value: number) => Math.round(value * 1000) / 1000;

const format = ([l, c, h]: Oklch) => `oklch(${round3(l)} ${round3(c)} ${round3(h)})`;

function parse(color: string): Oklch {
  const [l, c, h] = color.slice('oklch('.length, -1).split(' ').map(Number);

  return [l, c, h];
}

/** A lower-case #rrggbb (or #rgb) colour, or null. */
function normalizeHex(value: string): string | null {
  const hex = value.trim().toLowerCase();

  if (/^#[0-9a-f]{6}$/.test(hex)) {
    return hex;
  }

  const short = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/.exec(hex);

  return short ? `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}` : null;
}

function hexToRgb(hex: string): [number, number, number] {
  return [1, 3, 5].map((start) => parseInt(hex.slice(start, start + 2), 16)) as [number, number, number];
}

function hexToOklch(hex: string): string {
  const [red, green, blue] = hexToRgb(hex).map((channel) => {
    const value = channel / 255;

    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });

  const long = Math.cbrt(0.4122214708 * red + 0.5363325363 * green + 0.0514459929 * blue);
  const medium = Math.cbrt(0.2119034982 * red + 0.6806995451 * green + 0.1073969566 * blue);
  const short = Math.cbrt(0.0883024619 * red + 0.2817188376 * green + 0.6299787005 * blue);

  const lightness = 0.2104542553 * long + 0.793617785 * medium - 0.0040720468 * short;
  const a = 1.9779984951 * long - 2.428592205 * medium + 0.4505937099 * short;
  const b = 0.0259040371 * long + 0.7827717662 * medium - 0.808675766 * short;

  let hue = (Math.atan2(b, a) * 180) / Math.PI;
  if (hue < 0) {
    hue += 360;
  }

  return format([lightness, Math.sqrt(a * a + b * b), hue]);
}

function toRgb(color: string): [number, number, number] {
  if (color.startsWith('#')) {
    return hexToRgb(color);
  }

  const [lightness, chroma, hue] = parse(color);
  const radians = (hue * Math.PI) / 180;
  const a = chroma * Math.cos(radians);
  const b = chroma * Math.sin(radians);

  const long = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const medium = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const short = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;

  return [
    4.0767416621 * long - 3.3077115913 * medium + 0.2309699292 * short,
    -1.2684380046 * long + 2.6097574011 * medium - 0.3413193965 * short,
    -0.0041960863 * long - 0.7034186147 * medium + 1.707614701 * short,
  ].map((value) => {
    const srgb = value <= 0.0031308 ? 12.92 * value : 1.055 * value ** (1 / 2.4) - 0.055;

    return Math.max(0, Math.min(255, Math.round(srgb * 255)));
  }) as [number, number, number];
}

export function toHex(color: string): string {
  return `#${toRgb(color)
    .map((channel) => channel.toString(16).padStart(2, '0'))
    .join('')}`;
}

function alpha(color: string, opacity: number): string {
  const [red, green, blue] = toRgb(color);

  return `rgb(${red} ${green} ${blue} / ${opacity})`;
}

function contrast(first: string, second: string): number {
  const luminance = (color: string) => {
    const [red, green, blue] = toRgb(color);

    return 0.2126 * (red / 255) ** 2.2 + 0.7152 * (green / 255) ** 2.2 + 0.0722 * (blue / 255) ** 2.2;
  };

  const [lighter, darker] = [luminance(first), luminance(second)].sort((x, y) => y - x);

  return (lighter + 0.05) / (darker + 0.05);
}

function shift(color: string, lightness: number, chroma: number): string {
  const [l, c, h] = parse(color);

  return format([Math.max(0, l + lightness), Math.max(0, c * chroma), h]);
}

/** The fill a preset paints buttons with: the swatches in the settings. */
export function accentSwatch(preset: AccentPreset): string {
  return preset === 'blue' ? DEFAULT_ACCENT : toHex(PALETTES[preset][700]);
}

/**
 * Filament's generated scale for the colour's hue, at the colour's own saturation, with the colour
 * itself as the fill (700) and its hover (800) derived from it.
 */
function paletteAround(hex: string): Palette {
  const color = hexToOklch(hex);
  const [, chroma, hue] = parse(color);
  const saturation = Math.min(1.6, chroma / GENERATED_CHROMA_700);
  const achromatic = chroma < 0.03;

  const palette = Object.fromEntries(
    Object.entries(GENERATED).map(([shade, [l, c]]) => [
      shade,
      shift(format([l, achromatic ? 0 : c, hue]), 0, saturation),
    ]),
  ) as unknown as Palette;

  palette[700] = color;
  palette[800] = shift(color, -0.065, 0.85);

  return palette;
}

/** The colour, darkened until white text on it is readable. */
function readableFill(color: string): string {
  let fill = color;

  while (contrast(fill, '#ffffff') < MIN_CONTRAST && parse(fill)[0] > 0.1) {
    fill = shift(fill, -0.01, 1);
  }

  return fill;
}

/** The accent for the panel's theme colour, or null for Theme v2's own blue. */
export function accentFor(themeColor: string | null | undefined): Accent | null {
  const hex = normalizeHex(themeColor ?? '');

  if (hex === null || hex === DEFAULT_ACCENT) {
    return null;
  }

  // A preset's swatch is that preset, with Tailwind's own palette.
  const preset = ACCENT_PRESETS.find((name) => accentSwatch(name) === hex);
  if (preset === 'blue') {
    return null;
  }

  const palette: Palette = preset ? PALETTES[preset] : paletteAround(hex);

  const fill = readableFill(palette[700]);
  const hover = fill === palette[700] ? palette[800] : shift(fill, -0.065, 0.85);

  const text = ([500, 400] as const).find((shade) => contrast(palette[shade], SURFACE) >= MIN_CONTRAST) ?? 300;
  const bright = palette[text];
  const glow = palette[(text - 100) as Shade];

  return {
    tokens: {
      '--phv2-accent': toHex(fill),
      '--phv2-accent-hover': toHex(hover),
      '--phv2-accent-bright': toHex(bright),
      '--phv2-accent-glow': toHex(glow),
      '--phv2-accent-soft': alpha(fill, 0.1),
      '--phv2-accent-tint': alpha(fill, 0.15),
      '--phv2-accent-edge': alpha(fill, 0.2),
      '--phv2-accent-icon': alpha(bright, 0.5),
      // Tailwind's blue utilities (text-blue-400 and the like) are the accent too.
      ...Object.fromEntries(Object.entries(palette).map(([shade, color]) => [`--color-blue-${shade}`, toHex(color)])),
    },
    palette: [
      palette[50],
      palette[100],
      palette[(text - 200) as Shade],
      glow,
      bright,
      palette[600],
      fill,
      hover,
      palette[900],
      palette[950],
    ].map(toHex) as unknown as MantineColorsTuple,
  };
}
