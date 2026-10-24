// plugged.host: the numbers on the server cards and the console's stat blocks, as the Pelican panel formats them (ThemeV2ServerEntry, console.js)
import { mapUnitToLocale } from '@/lib/format/size.ts';
import { getTranslations } from '@/providers/TranslationProvider.tsx';

/** How full a resource is: Pelican's ListServers::WARNING_THRESHOLD and DANGER_THRESHOLD. */
export const USAGE_WARNING_THRESHOLD = 0.7;
export const USAGE_DANGER_THRESHOLD = 0.9;

export type UsageLevel = 'normal' | 'warning' | 'danger';

export function usageLevel(used: number, limit: number): UsageLevel {
  if (limit <= 0) return 'normal';

  const ratio = used / limit;

  return ratio >= USAGE_DANGER_THRESHOLD ? 'danger' : ratio >= USAGE_WARNING_THRESHOLD ? 'warning' : 'normal';
}

/** The bar's width: whole percent, at most 100, 0 without a limit. */
export function usageBarPercent(used: number, limit: number): number {
  return limit > 0 ? Math.min(100, Math.max(0, Math.round((used / limit) * 100))) : 0;
}

/** A number in the user's language with at most `decimals` decimals ("12.5", German "12,5"). */
export function formatNumber(value: number, decimals = 2): string {
  try {
    return new Intl.NumberFormat(getTranslations().language, { maximumFractionDigits: decimals }).format(value);
  } catch {
    return new Intl.NumberFormat('en', { maximumFractionDigits: decimals }).format(value);
  }
}

function sizeParts(megabytes: number): [string, string] {
  return megabytes >= 1024
    ? [Number((megabytes / 1024).toFixed(1)).toString(), mapUnitToLocale('GiB')]
    : [Math.round(megabytes).toString(), mapUnitToLocale('MiB')];
}

/**
 * "1.2 / 2 GiB": a size used of a limit, both in the panel's megabytes, with the unit once when
 * both sides share it (and on the limit alone while nothing is used). Without a limit the used
 * side keeps its unit and the limit is `unlimited`.
 */
export function formatSizePair(usedMegabytes: number, limitMegabytes: number, unlimited: string): [string, string] {
  const [usedValue, usedUnit] = sizeParts(usedMegabytes);

  if (limitMegabytes <= 0) {
    return [`${usedValue} ${usedUnit}`, unlimited];
  }

  const [limitValue, limitUnit] = sizeParts(limitMegabytes);

  return [
    usedUnit === limitUnit || usedMegabytes === 0 ? usedValue : `${usedValue} ${usedUnit}`,
    `${limitValue} ${limitUnit}`,
  ];
}

const UPTIME_UNITS: [Intl.NumberFormatOptions['unit'], number][] = [
  ['year', 31_536_000_000],
  ['month', 2_592_000_000],
  ['week', 604_800_000],
  ['day', 86_400_000],
  ['hour', 3_600_000],
  ['minute', 60_000],
  ['second', 1_000],
];

/**
 * "2h 14m": the two largest whole units of a duration in milliseconds, narrow, in the user's
 * language (Carbon's diffForHumans(short: true, parts: 2), which Pelican shows uptimes with).
 */
export function formatUptimeShort(milliseconds: number, parts = 2): string {
  const language = getTranslations().language;
  const format = (value: number, unit: Intl.NumberFormatOptions['unit']) => {
    try {
      return new Intl.NumberFormat(language, { style: 'unit', unit, unitDisplay: 'narrow' }).format(value);
    } catch {
      return new Intl.NumberFormat('en', { style: 'unit', unit, unitDisplay: 'narrow' }).format(value);
    }
  };

  const found: string[] = [];
  let remaining = Math.max(0, milliseconds);

  for (const [unit, size] of UPTIME_UNITS) {
    if (found.length >= parts) break;

    const value = Math.floor(remaining / size);
    if (value > 0) {
      found.push(format(value, unit));
      remaining -= value * size;
    }
  }

  return found.length ? found.join(' ') : format(0, 'second');
}
