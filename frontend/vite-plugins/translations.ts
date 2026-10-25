import fs from 'node:fs';
import path from 'node:path';
import type { LanguageData } from 'shared';
import type { Plugin } from 'vite';
import { type Language, languagesModule, offeredLanguages } from './languages.ts'; // plugged.host

// plugged.host: the languages the panel offers, in the pickers' order (docs/features/languages.md). Only
// these are built, from every source below (public/translations, extensions, PANEL_EXTRA_TRANSLATIONS_DIR):
// another language's file stays in the tree, so upstream merges stay clean, but never reaches
// dist/translations, so the API refuses it (FRONTEND_LANGUAGES), the pickers and browser detection never
// see it and mails leave its strings out.
export const LANGUAGES: Record<string, Language> = {
  en: { name: 'English' },
  de: { name: 'Deutsch', admin: true },
  ar: { name: 'العربية', rtl: true },
  ru: { name: 'Русский' },
  tr: { name: 'Türkçe' },
  id: { name: 'Bahasa Indonesia' },
  pt: { name: 'Português (Brasil)', locale: 'pt-BR' },
  vi: { name: 'Tiếng Việt' },
  fr: { name: 'Français' },
  uk: { name: 'Українська' },
  uz: { name: 'Oʻzbekcha' },
};

const VIRTUAL_ID = 'virtual:languages';
const RESOLVED_ID = `\0${VIRTUAL_ID}`;

function mergeLanguageTranslations(translationsDir: string, language: string): Record<string, LanguageData> | null {
  const namespaces: Record<string, LanguageData> = {};
  let found = false;

  const topLevelFile = path.join(translationsDir, `${language}.json`);
  if (fs.existsSync(topLevelFile)) {
    namespaces[''] = JSON.parse(fs.readFileSync(topLevelFile, 'utf-8'));
    found = true;
  }

  const langDir = path.join(translationsDir, language);
  if (fs.existsSync(langDir) && fs.statSync(langDir).isDirectory()) {
    for (const file of fs.readdirSync(langDir)) {
      if (file.endsWith('.json')) {
        const content = fs.readFileSync(path.join(langDir, file), 'utf-8');
        namespaces[file.replace('.json', '')] = JSON.parse(content);
        found = true;
      }
    }
  }

  return found ? namespaces : null;
}

function discoverLanguages(translationsDir: string): string[] {
  if (!fs.existsSync(translationsDir)) return [];

  const languages = new Set<string>();

  for (const entry of fs.readdirSync(translationsDir)) {
    const fullPath = path.join(translationsDir, entry);

    if (entry.endsWith('.json')) {
      languages.add(entry.replace('.json', ''));
    } else if (fs.statSync(fullPath).isDirectory()) {
      languages.add(entry);
    }
  }

  return [...languages];
}

function discoverTranslationDirs(): string[] {
  const dirs: string[] = [];

  const mainDir = path.resolve(import.meta.dirname, '..', 'public/translations');
  if (fs.existsSync(mainDir)) dirs.push(mainDir);

  const extensionsDir = path.resolve(import.meta.dirname, '..', 'extensions');
  if (fs.existsSync(extensionsDir)) {
    for (const ext of fs.readdirSync(extensionsDir)) {
      const extTransDir = path.join(extensionsDir, ext, 'public/translations');
      if (fs.existsSync(extTransDir) && fs.statSync(extTransDir).isDirectory()) {
        dirs.push(extTransDir);
      }
    }
  }

  const extraDir = process.env.PANEL_EXTRA_TRANSLATIONS_DIR;
  if (extraDir && fs.existsSync(extraDir) && fs.statSync(extraDir).isDirectory()) {
    dirs.push(path.resolve(extraDir));
  }

  return dirs;
}

function discoverAllLanguages(translationDirs: string[]): string[] {
  const languages = new Set<string>();
  for (const dir of translationDirs) {
    for (const lang of discoverLanguages(dir)) {
      languages.add(lang);
    }
  }
  return offeredLanguages(LANGUAGES, languages); // plugged.host: the allow-list
}

function mergeAllTranslations(translationDirs: string[], language: string): Record<string, LanguageData> | null {
  const merged: Record<string, LanguageData> = {};
  let found = false;

  for (const dir of translationDirs) {
    const result = mergeLanguageTranslations(dir, language);
    if (!result) continue;
    found = true;

    for (const [namespace, data] of Object.entries(result)) {
      if (merged[namespace]) {
        merged[namespace] = { ...merged[namespace], ...data };
      } else {
        merged[namespace] = data;
      }
    }
  }

  return found ? merged : null;
}

export function translationsPlugin(): Plugin {
  return {
    name: 'translations',

    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const match = req.url?.match(/^\/translations\/([^/]+)\.json$/);
        if (!match) return next();

        const language = match[1];
        // plugged.host: not even the public copy of a language off the list
        if (!Object.hasOwn(LANGUAGES, language)) {
          res.statusCode = 404;
          return res.end();
        }

        try {
          const dirs = discoverTranslationDirs();
          const merged = mergeAllTranslations(dirs, language);
          if (!merged) return next();

          res.setHeader('Content-Type', 'application/json');
          res.end(JSON.stringify(merged));
        } catch (err) {
          console.error(`[translations] Dev serve error for "${language}":`, err);
          next();
        }
      });
    },

    // plugged.host: `virtual:languages`, the built languages and their locale loaders (languages.ts)
    resolveId(id) {
      if (id === VIRTUAL_ID) return RESOLVED_ID;
    },

    load(id) {
      if (id === RESOLVED_ID) {
        return languagesModule(
          LANGUAGES,
          discoverAllLanguages(discoverTranslationDirs()),
          path.resolve(import.meta.dirname, '..', 'node_modules'),
        );
      }
    },

    closeBundle() {
      const outDir = path.resolve(import.meta.dirname, '..', 'dist/translations');
      const dirs = discoverTranslationDirs();
      const languages = discoverAllLanguages(dirs);

      // plugged.host: the public directory copy brought every language's file along; drop those off the list
      if (fs.existsSync(outDir)) {
        const dropped: string[] = [];
        for (const entry of fs.readdirSync(outDir)) {
          const language = entry.replace(/\.json$/, '');
          if (languages.includes(language)) continue;

          fs.rmSync(path.join(outDir, entry), { recursive: true, force: true });
          dropped.push(language);
        }
        if (dropped.length > 0) {
          console.log(`[translations] Not offered, left out: ${dropped.sort().join(', ')}`);
        }
      }

      if (languages.length === 0) return;

      let totalSource = 0;
      let totalMinified = 0;

      for (const language of languages) {
        try {
          const merged = mergeAllTranslations(dirs, language);
          if (!merged) continue;

          const minified = JSON.stringify(merged);

          for (const dir of dirs) {
            const topLevel = path.join(dir, `${language}.json`);
            if (fs.existsSync(topLevel)) {
              totalSource += fs.readFileSync(topLevel, 'utf-8').length;
            }
            const langDir = path.join(dir, language);
            if (fs.existsSync(langDir) && fs.statSync(langDir).isDirectory()) {
              for (const file of fs.readdirSync(langDir)) {
                if (file.endsWith('.json')) {
                  totalSource += fs.readFileSync(path.join(langDir, file), 'utf-8').length;
                }
              }
            }
          }

          fs.writeFileSync(path.join(outDir, `${language}.json`), minified);
          totalMinified += minified.length;
        } catch (err) {
          console.error(`[translations] Build error for "${language}":`, err);
        }
      }

      console.log(
        `[translations] Merged ${languages.length} languages from ${dirs.length} source(s) into dist/translations/`,
      );
      console.log(`[translations] Source: ${totalSource} bytes → Minified: ${totalMinified} bytes`);
    },
  };
}
