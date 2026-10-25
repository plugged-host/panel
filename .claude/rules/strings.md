---
paths:
  - "frontend/src/translations.ts"
  - "frontend/public/translations/**"
  - "shared/mails/**"
---

# Strings

- English is `frontend/src/translations.ts`; `pnpm build:translations` regenerates
  `public/translations/en.json` (commit it, never edit it by hand).
- German is kept at 100%: every new or changed English string gets its German at the same key in
  `public/translations/de.json` in the same commit, inserted after its English neighbour so the
  file keeps en's order. Informal "du", upstream's terms ("Node", "Allocation", "Egg", "Backup"
  stay; "Server suspendieren").
- Mails: `shared/mails/variables/<lang>.json`, English and German together.
- The offered languages are `LANGUAGES` in `frontend/vite-plugins/translations.ts` (en, de, ar, ru,
  tr, id, pt = Brazilian, vi, fr, uk, uz); others are never built. The nine besides German need
  every string except admin-only ones (a key with an `admin` segment) before the next deploy, with
  their glossary's words (docs/features/languages.md).
- `pnpm coverage:translations` (all offered; `de --missing` lists one's keys) must pass; the build
  refuses otherwise. Never edit an off-list language's file (cs, da, es, it, ja, lv, pl, ro, sk, sv,
  zh): it is upstream's.
- Keys follow the native tree (`pages.server.backups.toast.locked`); no namespace of our own.
- Upstream's Crowdin updates come in through the `translations` merge driver
  (`frontend/merge-translations.mjs`, `.gitattributes`, tests: `pnpm test:translations`), which
  merges by key on `git merge upstream/main`.
