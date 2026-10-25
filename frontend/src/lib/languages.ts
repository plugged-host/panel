// plugged.host: the offered languages (vite-plugins/translations.ts) as the pickers show them.
import { languages } from 'virtual:languages';

/** A language's name in its own language ("Deutsch", "العربية"). */
export function languageName(code: string): string {
  if (Object.hasOwn(languages, code)) return languages[code].name;

  try {
    return new Intl.DisplayNames([code], { type: 'language' }).of(code) ?? code;
  } catch {
    return code;
  }
}

/** Picker options for the panel's languages (the store's `languages`), in the allow-list's order. */
export function languageOptions(codes: string[]): { label: string; value: string }[] {
  const order = Object.keys(languages);
  const rank = (code: string) => (order.includes(code) ? order.indexOf(code) : order.length);

  return [...codes].sort((a, b) => rank(a) - rank(b)).map((code) => ({ label: languageName(code), value: code }));
}

/**
 * The locale numbers and dates are formatted in: the language with Latin digits, so Arabic shows 0-9
 * like its translations do instead of its default ٠-٩ (the other languages default to 0-9 already).
 */
export function intlLocale(code: string): string {
  return `${code}-u-nu-latn`;
}
