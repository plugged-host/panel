# API documentation

The API documentation pages the Pelican panel has (its Scramble pages), generated from the routes,
so they always describe the running panel.

| Page | What |
|---|---|
| `/docs/api` | The list of APIs |
| `/docs/api/client` | The client API (`/api/client/…`) in Stoplight Elements, with "Try It" |
| `/docs/api/admin` | The admin API (`/api/admin/…`) |
| `/docs/api/client.json`, `/docs/api/admin.json` | Their OpenAPI documents |
| `/docs/api/application`, `/docs/api/application.json` | Pelican's name for the admin API: a permanent redirect |

`/openapi.json` (upstream) stays the whole document. Each API's document is cut from it at startup
(`backend/src/routes/docs.rs`): its paths, the components they use (followed through `$ref`s), the
security schemes, and the panel's name in the title. Every route's `#[utoipa::path]` is what shows,
so a new route is documented by writing it the way `.claude/rules/routes.md` says.

Like on Pelican, only signed-in users (not suspended) see them: without a session `/docs/api` says
so with a link to sign in, the API pages redirect there and the JSON answers `401`. "Try It" sends
the browser's own session cookie, or an API key in `Authorization: Bearer <key>`.

Stoplight Elements 8.4.2 is loaded from unpkg like Scramble does, pinned with subresource
integrity (`ELEMENTS_JS_SRI`, `ELEMENTS_CSS_SRI`; update both with the version). The pages have their
own Content Security Policy: scripts only from the panel and unpkg, requests only to the panel.

## Check

- Signed in, open `/docs/api`: two links; Client API lists the server and account routes, Admin API
  the admin routes; "Try It" on `GET /api/client/account` answers with your account.
- Signed out (private window): `/docs/api` asks to sign in; `/docs/api/client.json` is `401`.
- `/docs/api/application` lands on the admin API.
