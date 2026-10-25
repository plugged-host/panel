// plugged.host: tests for the language rules (`node --test vite-plugins/languages.test.ts`).
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import {
  isAdminKey,
  type Language,
  languagesModule,
  missingKeys,
  offeredLanguages,
  requiredKeys,
} from './languages.ts';
import { LANGUAGES } from './translations.ts';

test('an admin-only string is a key with an admin segment', () => {
  assert.equal(isAdminKey('translations.pages.admin.nodes.title'), true);
  assert.equal(isAdminKey('translations.elements.economyTransactions.type.admin'), true);
  assert.equal(isAdminKey('translations.pages.server.viewAdmin.title'), false);
  assert.equal(isAdminKey('translations.pages.auth.login.error.noprofitAdminsOnly'), false);
  assert.equal(isAdminKey('items.server.one'), false);
});

test('a language without admin strings may leave them to English; empty counts as missing', () => {
  const english = {
    'translations.pages.server.title': 'Server',
    'translations.pages.admin.title': 'Admin',
    'items.server.one': '{count} server',
  };
  const translated = { 'translations.pages.server.title': 'Сервер', 'items.server.one': '' };

  assert.deepEqual(requiredKeys(english, false), ['translations.pages.server.title', 'items.server.one']);
  assert.deepEqual(missingKeys(english, translated, false), ['items.server.one']);
  assert.deepEqual(missingKeys(english, translated, true), ['translations.pages.admin.title', 'items.server.one']);
});

test("only offered languages are built, in the list's order", () => {
  const languages: Record<string, Language> = { en: { name: 'English' }, de: { name: 'Deutsch' }, ar: { name: 'ar' } };

  assert.deepEqual(offeredLanguages(languages, ['pl', 'ar', 'en', 'it', 'de']), ['en', 'de', 'ar']);
  assert.deepEqual(offeredLanguages(languages, new Set(['de', 'cs'])), ['de']);
});

test('German is kept complete, English is the source, Arabic runs right to left', () => {
  assert.ok(Object.hasOwn(LANGUAGES, 'en'));
  assert.equal(LANGUAGES.de.admin, true);
  assert.deepEqual(
    Object.keys(LANGUAGES).filter((code) => LANGUAGES[code].rtl),
    ['ar'],
  );
});

test('the virtual module loads the locale files of built languages only', async () => {
  const nodeModules = fs.mkdtempSync(path.join(os.tmpdir(), 'languages-'));
  const touch = (file: string) => {
    fs.mkdirSync(path.dirname(path.join(nodeModules, file)), { recursive: true });
    fs.writeFileSync(path.join(nodeModules, file), '');
  };

  try {
    for (const file of [
      'zod/v4/locales/en.js',
      'zod/v4/locales/pt-BR.js',
      'zod/v4/locales/uz.js',
      'zod/v4/locales/pl.js',
      'monaco-editor/esm/vs/nls/lang/pt-br.js',
      'monaco-editor/esm/vs/nls/lang/pl.js',
      'cronstrue/locales/pt_BR.js',
      'cronstrue/locales/pl.js',
    ]) {
      touch(file);
    }

    const languages: Record<string, Language> = {
      en: { name: 'English' },
      pt: { name: 'Português (Brasil)', locale: 'pt-BR' },
      uz: { name: 'Oʻzbekcha' },
      ar: { name: 'العربية', rtl: true },
    };
    const source = languagesModule(languages, ['en', 'pt', 'uz', 'ar'], nodeModules);
    const file = path.join(nodeModules, 'module.mjs');
    fs.writeFileSync(file, source.replaceAll(/\(\) => import\(("[^"]+")\)/g, '() => $1'));
    const module = await import(file);

    assert.deepEqual(module.languages, {
      en: { name: 'English', locale: 'en', dir: 'ltr' },
      pt: { name: 'Português (Brasil)', locale: 'pt-BR', dir: 'ltr' },
      uz: { name: 'Oʻzbekcha', locale: 'uz', dir: 'ltr' },
      ar: { name: 'العربية', locale: 'ar', dir: 'rtl' },
    });
    assert.deepEqual(Object.keys(module.zodLocales), ['en', 'pt', 'uz']);
    assert.equal(module.zodLocales.pt(), path.join(nodeModules, 'zod/v4/locales/pt-BR.js'));
    assert.deepEqual(Object.keys(module.monacoLocales), ['pt']);
    assert.equal(module.monacoLocales.pt.locale, 'pt-br');
    assert.deepEqual(Object.keys(module.cronstrueLocales), ['pt']);
    assert.equal(module.cronstrueLocales.pt.name, 'pt_BR');
    assert.ok(!source.includes('pl.js'));
  } finally {
    fs.rmSync(nodeModules, { recursive: true, force: true });
  }
});
