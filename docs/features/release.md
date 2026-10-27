# Release

Every build knows its release, `v{VERSION}` (for example `v3.0.0`): the `VERSION` file at the
repository root. It is how plugged.host names what runs where, across the panel and its other
services.

There is no commit in the release: the panel is a compiled Rust binary, so what runs is whatever
was built and deployed, not a checkout. A fix for a released line is a new version: bump `VERSION`
(`3.0.0` → `3.0.1`), build and deploy. Each release has a GitHub release of the same name with the
panel and Wings binaries.

| Where | What |
|---|---|
| `VERSION` | The version (`3.0.0`), bumped by hand for every release, fixes included |
| `shared/build.rs` | Reads `VERSION` at build time (`PANEL_RELEASE` overrides it) |
| `shared::RELEASE`, `shared::RELEASE_VERSION` | The release and the version in code |
| `GET /api/settings` → `app.release` | For the frontend |
| `frontend/src/elements/Copyright.tsx` | The footer shows it |
| `panel-rs version` | Prints `release v3.0.0`, then Calagopus's own version |
| `plugged-host/deploy.sh` | Installs each deploy in `releases/<release>-<UTC time>`, so redeploying a release keeps the previous binary for rollback |

Cargo's package version stays upstream's (`shared::VERSION`), so error reports and extension
compatibility keep using Calagopus's numbers.

## Check

- The footer shows the release of the running build.
- `panel-rs version` prints the same.
