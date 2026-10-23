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
- `pnpm coverage:translations de` (`--missing` lists keys) must pass; the build refuses otherwise.
- Keys follow the native tree (`pages.server.backups.toast.locked`); no namespace of our own.
- Upstream's other languages come from its Crowdin; the `translations` merge driver
  (`frontend/merge-translations.mjs`, `.gitattributes`) merges them by key on `git merge upstream/main`.
