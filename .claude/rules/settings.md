---
paths:
  - "shared/src/settings/**"
  - "backend/src/routes/api/admin/settings.rs"
  - "backend/src/routes/api/settings.rs"
  - "frontend/src/pages/admin/settings/**"
---

# Settings

- A section is one file in `shared/src/settings/<section>.rs` (struct + serializer + deserializer,
  copy `noprofit.rs` or `webauthn.rs`), a field and (de)serializer entry in `mod.rs` marked
  `// plugged.host`, a payload struct in `backend/src/routes/api/admin/settings.rs`, and an Admin ›
  Settings tab in `frontend/src/pages/admin/settings/<section>/` (container + form values, a zod
  schema in `lib/schemas/admin/settings.ts`, store defaults in `stores/slices/admin/settings.ts`,
  the form id in `elements/form-engine/types.ts`).
- A section with a bigger payload keeps it in its own file with an `apply(self, &mut section)` that
  validates across fields and returns the error (`games.rs`, `placement.rs`); the admin route then
  only adds a field and one `apply` call.
- Keys are `section::field`. Defaults are neutral and off for anything optional.
- Secrets: `write_raw_encrypted_setting` / `database.decrypt_base64`, serialized as `""` when set and
  `null` when not; the admin API treats `""` as "keep" and `null` as "clear".
- Everything an admin may change is a setting; never ask anyone to edit the environment.
