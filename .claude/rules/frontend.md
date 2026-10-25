---
paths:
  - "frontend/src/**"
---

# Frontend

- Build pages from `src/elements` and the hooks in `src/plugins` (`useSearchablePaginatedTable`,
  `useResourceForm`, `useModalForm`, `usePermissions`). Pages live in
  `src/pages/{server,dashboard,admin}/<feature>/`, API calls in `src/api/{server,me,admin}/<feature>/`,
  routes and sidebar entries in `src/routers/routes/*.ts`, extra tabs in the view's own tab list.
- Never hard-code text: `t('pages.<area>.<feature>.<key>', {})`, with the English in
  `src/translations.ts` and the German in `public/translations/de.json` in the same change.
- The look comes from Theme v2 (`app.css`, `theme.ts`). Use the `--theme-*` tokens and Mantine's
  palette; no new colours, gradients, shadows, glass effects or decorative icons. One filled button
  per section; destructive actions are secondary until their confirmation.
- Import icons by their FontAwesome names; they are drawn as Tabler icons (`src/lib/icons/`). Add
  the Tabler equivalent to `tablerIcons.ts` when one is missing.
- CSS uses logical properties (`margin-inline-start`, `padding-inline-end`, `text-align: start`;
  Tailwind `ms-*`, `pe-*`, `start-*`, `text-start`) so Arabic, right to left, needs no rework. Paths,
  addresses, ports and code get `dir='ltr'`; a new directional icon goes into the mirror rule at the
  end of `app.css` (docs/features/languages.md#right-to-left).
- Language names and the list come from `virtual:languages` / `src/lib/languages.ts`, never from a
  list of codes of your own.
- Run `pnpm typecheck` and `pnpm biome:validate`; don't start a browser to check.
