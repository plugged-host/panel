# Release

Every build knows its release, `v{VERSION}.{commit}` (for example `v3.0.0.7bc33d4`): the `VERSION`
file at the repository root plus the 7-character commit it was built from. It is how plugged.host
names what runs where, across the panel and its other services.

| Where | What |
|---|---|
| `VERSION` | The version (`3.0.0`), bumped by hand for a release line |
| `shared/build.rs` | Reads `VERSION` and the commit at build time (`PANEL_RELEASE` overrides both) |
| `shared::RELEASE`, `shared::RELEASE_VERSION` | The release and the version in code |
| `GET /api/settings` → `app.release` | For the frontend |
| `frontend/src/elements/Copyright.tsx` | The footer shows it |
| `panel-rs version` | Prints `release v3.0.0.7bc33d4`, then Calagopus's own version |

Cargo's package version stays upstream's (`shared::VERSION`), so error reports and extension
compatibility keep using Calagopus's numbers.

Build from a committed tree: the release names the commit, not uncommitted changes.

## Check

- The footer shows the release of the running build.
- `panel-rs version` prints the same.
