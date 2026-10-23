// plugged.host: how complete a language is against English, the panel's strings and the mails'.
//
//   pnpm coverage:translations de            # exits 1 below 100%
//   pnpm coverage:translations de --missing  # also lists the missing keys
//
// Run `pnpm build:translations` first: it writes public/translations/en.json from src/translations.ts.
import fs from 'node:fs/promises';

const args = process.argv.slice(2);
const listMissing = args.includes('--missing');
const languages = args.filter((arg) => !arg.startsWith('--'));

if (languages.length === 0) {
  console.error('Syntax: pnpm coverage:translations <language>... [--missing]');
  process.exit(1);
}

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
  for (const [name, path] of Object.entries(sources)) {
    const english = await read(path('en'));
    const translated = await read(path(language));
    const missing = Object.keys(english).filter((key) => translated[key] === undefined || translated[key] === '');
    const total = Object.keys(english).length;
    // rounded down, so a single missing key never shows as 100%
    const share = Math.floor((1000 * (total - missing.length)) / Math.max(total, 1)) / 10;

    console.log(`${language} ${name}: ${share.toFixed(1)}% (${missing.length} of ${total} missing)`);
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
