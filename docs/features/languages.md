# Languages

The panel speaks eleven languages: English (the source), German, kept complete, and nine languages
chosen for where plugged.host's visitors read English least. Every other language Calagopus ships is
switched off completely: it is not built, not offered and not accepted by the API.

## Offered

| Code | Language | The picker shows | Notes |
|---|---|---|---|
| `en` | English | English | The source: `frontend/src/translations.ts` |
| `de` | German | Deutsch | Every string, admin pages and mails included |
| `ar` | Arabic | العربية | Right to left ([below](#right-to-left)) |
| `ru` | Russian | Русский | |
| `tr` | Turkish | Türkçe | |
| `id` | Indonesian | Bahasa Indonesia | |
| `pt` | Portuguese (Brazil) | Português (Brasil) | Brazilian Portuguese in `pt.json`; locale `pt-BR` |
| `vi` | Vietnamese | Tiếng Việt | |
| `fr` | French | Français | |
| `uk` | Ukrainian | Українська | |
| `uz` | Uzbek | Oʻzbekcha | Latin script, the official one |

Each name is written in its own language, in the order above. Where a library has no locale for a
language its own text stays English: cron descriptions on the schedules pages for Uzbek (cronstrue
has none), the file editor's menus for Arabic, Indonesian, Vietnamese, Ukrainian and Uzbek (Monaco
has none). Form validation messages come from zod, which has all of them.

## One list, everything else off

The list is `LANGUAGES` in `frontend/vite-plugins/translations.ts`: the code, the name, and when
needed the BCP 47 `locale`, `rtl` and `admin` (admin strings must be translated too). The rest
follows from it:

| Where | What the list does |
|---|---|
| Frontend build (the same plugin) | Only listed languages are merged into `dist/translations/`, from every source: `public/translations/`, extensions' translations and `PANEL_EXTRA_TRANSLATIONS_DIR` (the heavy image's custom translations). The public directory's copies of the others are deleted from `dist/` |
| `virtual:languages` | The built languages with their names, locales and directions, and loaders for their zod, Monaco and cronstrue locale files only; no other language's locale file is bundled |
| API | `shared::FRONTEND_LANGUAGES` is read from the built files, so `GET /api/languages` lists exactly these and `validate_language` refuses any other code: an account's language, registration, the admin user form, the default language (`settings.app.language`), mail variable translations |
| Browser detection | `TranslationProvider` picks the first browser language on the list (`pt-PT` and `pt-BR` both find `pt`); a browser set to Polish opens in the default language |
| Stored choices | A language off the list in the browser's `last_language` or on an account from before opens in the default language |
| Mails | `shared/mails/variables/<code>.json` is read for listed languages only (`shared::utils::language_offered`) |

Upstream's files for the other languages (cs, da, es, it, ja, lv, pl, ro, sk, sv, zh, and whatever
Crowdin adds) stay in the tree untouched, so merges stay clean; they are never built. Migration
`languages` moved accounts holding one of them to the default language.

## Where the strings live

- English: `frontend/src/translations.ts`; `pnpm build:translations` writes `public/translations/en.json`.
- Every other language: `frontend/public/translations/<code>.json` (upstream's file where it has one),
  edited directly, in English's key order. Mails: `shared/mails/variables/<code>.json`.
- An extension's strings: `public/translations/<code>/<namespace>.json` in the extension.

Upstream's Crowdin updates reach the same files through the `translations` merge driver, which
merges them key by key ([upstream.md](../upstream.md#translations)).

## Coverage

```bash
cd frontend
pnpm build:translations
pnpm coverage:translations               # every offered language but English
pnpm coverage:translations ru --missing  # one language, with the keys it lacks
```

- **German** must have every key: the panel's strings, admin pages and mails.
- **The nine others** must have every key except the admin-only ones. An admin-only string is a key
  with an `admin` segment (`translations.pages.admin.…`, `…economyTransactions.type.admin`); plugged.host
  writes admin strings in English and German only, upstream's follow its Crowdin.
- Plural items count per form (`items.server.few`), so a language has every form English has.
- An empty string counts as missing.

At runtime anything missing falls back to English: a missing or empty string, a missing item and a
missing plural form (`TranslationProvider`'s `t`, `tReact`, `tItem`).

`plugged-host/translations.sh` and `plugged-host/build.sh` run the check for every offered language
(`LANGUAGES='de ru'` narrows it) and stop below 100%. A new English string therefore needs all ten
translations before the next deploy, and so do the strings an upstream merge brings in: fill them
before deploying the merge.

## Adding or removing a language

1. Add it to `LANGUAGES` with its own name; `locale` when its locale files use a regional tag
   (`pt-BR`), `rtl: true` for a right-to-left script.
2. Write its glossary down next to these docs before translating (below).
3. Create `frontend/public/translations/<code>.json` (or keep upstream's) and
   `shared/mails/variables/<code>.json`, and translate until
   `pnpm coverage:translations <code>` passes.
4. zod, Monaco and cronstrue locales are found by themselves (the `locale`, then the base language);
   with none the library stays English.
5. Go through the check list below in that language.

Removing one is the reverse: take it off the list and add a data migration that moves its accounts
to the default language, as `languages` did.

## Glossary

Each language has one glossary, and every translator and every later string uses it:

- Names stay as they are: plugged.host, Plus, Premium, neo, rux, and the product names an admin
  sets (they are settings, not strings).
- The panel's own terms get one fixed word each: tokens, credits, node, allocation, membership,
  server, backup, egg. Where upstream's file already uses a word, upstream's word wins, since
  people already see it.
- The tone follows upstream's file (German uses the informal "du").
- Placeholders (`{count}`, `{{ vars.x }}`), Markdown, paths, commands and units stay as in English.

## Right to left

- `TranslationProvider` sets `<html lang dir>` from the list when a language loads; Mantine's
  `DirectionProvider` follows the attribute, so its drawers, menus and popovers open from the right
  side.
- The layout mirrors because the CSS is logical: `margin-inline-*`, `padding-inline-*`,
  `inset-inline-*`, `text-align: start` in CSS, `ms-*`/`me-*`/`ps-*`/`pe-*`/`start-*`/`end-*`/
  `text-start` in Tailwind. Use them for every new rule; a physical `ml-*` or `left:` is wrong in
  Arabic.
- Icons that point along the line (chevrons, arrows, log in/out, send) mirror through one rule in
  `app.css` (by their Tabler class). A new directional icon goes into that rule. FontAwesome's
  `rotation` keeps working; a Tailwind `rotate-90` on such an icon needs `rtl:-rotate-90` beside it.
- Technical text stays left to right: the console and its command line, the Monaco and Pierre
  editors, database logs, `pre`/`code`/`kbd`/`samp` (CSS), breadcrumbs (they are paths), copyable
  values on stat cards, server addresses and the SFTP host and username. Give a new technical value
  (a path, an address, a port, a key) `dir='ltr'`.
- Scrolling names scroll the other way, and the welcome tour's card sits beside the sidebar on the
  right.
- Numbers and dates use Latin digits (0-9) in every language, as the translations do: format them
  with `intlLocale(language)` from `lib/languages.ts`, not the bare language (Arabic's default is ٠-٩).

Upstream's own components still use physical classes in about two hundred places (`ml-2`,
`text-left`); they are left as upstream wrote them, to keep merges small, and only shift a margin.

## Check

- With the browser set to Turkish (and no language chosen before), a first visit opens in Turkish;
  set to Polish, it opens in the default language.
- The pickers (sign-in page, account settings, Admin › Users, Admin › Settings › General ›
  Application, the first-run setup) show English, Deutsch and the nine names above in that order,
  nothing else.
- Setting `pl` or `it` through `PATCH /api/client/account` or as the default language is refused.
- In each language, no English is left on the dashboard, console, create server, queue and waiting
  room, and the Plus and store pages; admin pages may be English except in German.
- German has no English left anywhere, admin pages and mails included.
- Arabic mirrors the whole layout (sidebar on the right, chevrons pointing left); the console, the
  file editor, file paths and addresses read left to right.
- A schedule's cron description is in the panel's language (English in Uzbek).
- A panel mail arrives in the user's language.
