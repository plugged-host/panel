---
paths:
  - "shared/src/permissions.rs"
  - "frontend/src/plugins/usePermissions.ts"
  - "frontend/src/routers/routes/**"
---

# Permissions

- New permissions go into upstream's groups when they fit (`control.*`, `settings.*`, `files.*`);
  a new area gets its own group named like upstream's (`status`, `abuse`, `minecraft`).
- `REMOVED_USER_PERMISSIONS` / `REMOVED_ADMIN_PERMISSIONS` / `REMOVED_SERVER_PERMISSIONS` are the
  features this fork removed. `frontend/src/plugins/usePermissions.ts` mirrors them: change both
  together, and add a migration that strips a newly removed permission from roles, API keys and
  subusers.
- Route entries in `routers/routes/*.ts` gate on a permission like upstream's; use `filter` for
  entries that depend on settings (for example noprofit.space being connected).
