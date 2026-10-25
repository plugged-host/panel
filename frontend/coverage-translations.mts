// plugged.host: how complete a language is against English, the panel's strings and the mails'.
//
//   pnpm coverage:translations               # every offered language but English; exits 1 below 100%
//   pnpm coverage:translations de ru         # only these
//   pnpm coverage:translations de --missing  # also lists the missing keys
//
// A language marked `admin` in the allow-list (vite-plugins/translations.ts: German) needs every key;
// the others every key but the admin-only ones (a key with an `admin` segment), which fall back to
// English at runtime. Run `pnpm build:translations` first: it writes public/translations/en.json.
import fs from 'node:fs/promises';
import { missingKeys, requiredKeys } from './vite-plugins/languages.ts';
import { LANGUAGES } from './vite-plugins/translations.ts';

const args = process.argv.slice(2);
const listMissing = args.includes('--missing');
const named = args.filter((arg) => !arg.startsWith('--'));
const languages = named.length > 0 ? named : Object.keys(LANGUAGES).filter((language) => language !== 'en');

const flatten = (value: unknown, prefix = '', out: Record<string, unknown> = {}): Record<string, unknown> => {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    for (const [key, item] of Object.entries(value)) {
      flatten(item, prefix ? `${prefix}.${key}` : key, out);
    }
  } else {
    out[prefix] = value;
  }

  return out;
};

const read = async (path: string): Promise<Record<string, unknown>> => {
  try {
    return flatten(JSON.parse(await fs.readFile(path, 'utf8')));
  } catch {
    return {};
  }
};

const sources = {
  panel: (language: string) => `public/translations/${language}.json`,
  mails: (language: string) => `../shared/mails/variables/${language}.json`,
};

let complete = true;
for (const language of languages) {
  if (!Object.hasOwn(LANGUAGES, language)) {
    console.log(`${language}: not offered (vite-plugins/translations.ts), checked without its admin strings`);
  }
  const admin = Object.hasOwn(LANGUAGES, language) && LANGUAGES[language].admin === true;

  for (const [name, path] of Object.entries(sources)) {
    const english = await read(path('en'));
    const translated = await read(path(language));
    const missing = missingKeys(english, translated, admin);
    const total = requiredKeys(english, admin).length;
    const fallback = Object.keys(english).length - total;
    // rounded down, so a single missing key never shows as 100%
    const share = Math.floor((1000 * (total - missing.length)) / Math.max(total, 1)) / 10;
    const note = fallback > 0 ? `; ${fallback} admin strings not required, English where missing` : '';

    console.log(`${language} ${name}: ${share.toFixed(1)}% (${missing.length} of ${total} missing${note})`);
    if (missing.length > 0) {
      complete = false;
      if (listMissing) {
        for (const key of missing) {
          console.log(`  ${key}: ${JSON.stringify(english[key])}`);
        }
      }
    }
  }
}

process.exit(complete ? 0 : 1);
