---
paths:
  - "backend/src/routes/**"
---

# Routes

- Place routes in the existing tree next to their siblings (`client/servers/_server_/<feature>/`,
  `client/account/<feature>/`, `admin/<feature>/`, `admin/nodes/_node_/<feature>/`) and mount them
  with `.nest(...)` in the parent's `router()`, like upstream's. No route prefixes of our own.
- One `mod get|post|patch|delete { pub async fn route(...) }` per endpoint with a `#[utoipa::path]`,
  so it appears in the OpenAPI spec. Payloads derive `garde::Validate`; responses are `ToSchema`.
- Check the permission at the top of the handler (`permissions.has_admin_permission("x.y")?`,
  `permissions.has_server_permission("x.y")?`) and log changes with the matching activity logger.
- User errors: `ApiResponse::error("lower case, no trailing period").with_status(StatusCode::...)`.
- Removed upstream features are unmounted by commenting the `mod` and `.nest` lines with a
  `// plugged.host: <feature> removed (docs/upstream.md)` marker; keep them that way.
- Admin settings changes go through the section's payload in `admin/settings.rs`; what the frontend
  may read before sign-in goes in `api/settings.rs`.
