---
paths:
  - "shared/src/models/**"
  - "shared/src/*.rs"
  - "backend/src/tasks.rs"
---

# Models and database code

- Runtime `sqlx` only: `sqlx::query`, `query_as`, `query_scalar` with `.bind(...)`. Never the
  `query!` / `query_as!` macros (they need `cargo sqlx prepare` and a live database).
- SQL built with `format!` goes through `sqlx::AssertSqlSafe(format!(...))`, and only identifiers or
  constant fragments are formatted in; values are always bound.
- Writes on `state.database.write()`, reads on `state.database.read()`.
- A model implements upstream's traits (`BaseModel`, `ByUuid`, `CreatableModel`, `UpdatableModel`,
  `DeletableModel`, `IntoApiObject` / `IntoAdminApiObject`) like `shared/src/models/location.rs`,
  and is registered in `shared/src/models/mod.rs`.
- React to other models through their event handlers (`register_create_handler`,
  `register_delete_handler`, …) instead of editing the code that creates or deletes them.
- Shared logic for a feature is one file at `shared/src/` root (`start_gates.rs`, `abuse.rs`), with
  its unit tests in a `#[cfg(test)] mod tests` at the bottom.
- Background tasks go in `define_background_tasks` (`backend/src/tasks.rs`) next to upstream's; they
  run on the primary instance only. Drop a settings read guard before awaiting anything else.
