// plugged.host: the rules behind the language allow-list in translations.ts (docs/features/languages.md).
import fs from 'node:fs';
import path from 'node:path';

export type Language = {
  /** The language's name in its own language, as the pickers show it. */
  name: string;
  /** The BCP 47 tag for `<html lang>` and the zod, Monaco and cronstrue locale files, when it differs from the code. */
  locale?: string;
  /** Written right to left. */
  rtl?: boolean;
  /** Admin-only strings are translated too, so coverage counts them; without it they fall back to English. */
  admin?: boolean;
};

/** An admin-only string: a key with an `admin` segment (`translations.pages.admin.…`). */
export function isAdminKey(key: string): boolean {
  return key.split('.').includes('admin');
}

/** The keys a language must have: all of English's, without the admin-only ones unless `admin` is set. */
export function requiredKeys(english: Record<string, unknown>, admin: boolean): string[] {
  return Object.keys(english).filter((key) => admin || !isAdminKey(key));
}

/** The required keys the translation lacks or leaves empty. */
export function missingKeys(
  english: Record<string, unknown>,
  translated: Record<string, unknown>,
  admin: boolean,
): string[] {
  return requiredKeys(english, admin).filter((key) => translated[key] === undefined || translated[key] === '');
}

/** The offered languages among the ones found, in the list's order. */
export function offeredLanguages(languages: Record<string, Language>, found: Iterable<string>): string[] {
  const available = new Set(found);

  return Object.keys(languages).filter((code) => available.has(code));
}

const localeFile = (dir: string, candidates: string[]): { name: string; file: string } | null => {
  for (const name of candidates) {
    const file = path.join(dir, `${name}.js`);
    if (fs.existsSync(file)) return { name, file };
  }

  return null;
};

const loader = (file: string) => `() => import(${JSON.stringify(file.split(path.sep).join('/'))})`;

/**
 * The source of `virtual:languages`: the built languages (name, locale, direction) and loaders for the
 * zod, Monaco and cronstrue locale files of those languages only, so no other language's file is bundled.
 */
export function languagesModule(languages: Record<string, Language>, built: string[], nodeModules: string): string {
  const info: Record<string, { name: string; locale: string; dir: 'ltr' | 'rtl' }> = {};
  const zod: string[] = [];
  const monaco: string[] = [];
  const cronstrue: string[] = [];

  for (const code of built) {
    const language = languages[code];
    const locale = language.locale ?? code;
    const base = locale.split('-')[0];
    info[code] = { name: language.name, locale, dir: language.rtl ? 'rtl' : 'ltr' };

    const zodFile = localeFile(path.join(nodeModules, 'zod/v4/locales'), [locale, base]);
    if (zodFile) zod.push(`${JSON.stringify(code)}: ${loader(zodFile.file)}`);

    if (code === 'en') continue;

    const monacoFile = localeFile(path.join(nodeModules, 'monaco-editor/esm/vs/nls/lang'), [
      locale.toLowerCase(),
      base,
    ]);
    if (monacoFile) {
      monaco.push(
        `${JSON.stringify(code)}: { locale: ${JSON.stringify(monacoFile.name)}, load: ${loader(monacoFile.file)} }`,
      );
    }

    const cronstrueFile = localeFile(path.join(nodeModules, 'cronstrue/locales'), [locale.replace('-', '_'), base]);
    if (cronstrueFile) {
      cronstrue.push(
        `${JSON.stringify(code)}: { name: ${JSON.stringify(cronstrueFile.name)}, load: ${loader(cronstrueFile.file)} }`,
      );
    }
  }

  return [
    `export const languages = ${JSON.stringify(info)};`,
    `export const zodLocales = { ${zod.join(', ')} };`,
    `export const monacoLocales = { ${monaco.join(', ')} };`,
    `export const cronstrueLocales = { ${cronstrue.join(', ')} };`,
  ].join('\n');
}
