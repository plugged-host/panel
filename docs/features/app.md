# The Windows app

plugged.host has a native Windows client, `plugged-host/app`, written in Rust (egui). It is not a web
view: the panel's Theme v2 is drawn natively, and the app talks to the same `/api/client` endpoints
the browser does. It signs in with noprofit.space through a loopback listener (the browser hands a
signed token to `127.0.0.1`, the app trades it for a panel session at
`/api/auth/noprofit/app/exchange`).

## Download page

Account › More › **Download app** appears for **admins** on a panel where the installer exists.
The public settings carry `appDownload: { enabled, version }`; the row is rendered only when both
`isAdmin(user)` and `enabled` are true.

| Route | What |
|---|---|
| `GET /api/client/app` | The installer's version, filename and size (admins only; 404 otherwise) |
| `GET /api/client/app/download` | The installer, as an attachment (admins only) |

`PLUGGED_APP_INSTALLER` names the file on the box; when it is unset or missing, the feature is off
and both routes answer 404. `PLUGGED_APP_VERSION` labels it. `PLUGGED_APP_RELEASE_NOTES` is shown
on the page. Dev sets them; prod does not, so nothing is offered there.

The installer is built on ryzen by `plugged-host/app/build.sh` and copied to
`/var/lib/calagopus/app/plugged.host-Setup-<version>.exe` on dev.

## Sign-in

The desktop flow reuses the Calagopus SSO client's audience. accounts.noprofit.space has a
`plugged-app` client with the same secret and audience and a loopback-only callback
(`http://127.0.0.1:<port>/callback?state=…`). The panel stores the app's state in its cache for ten
minutes, verifies it once, decodes the token and starts the same session the browser would. Only
noprofit.space admins can use it while Admin › Settings › Accounts › noprofit.space is "admins
only".

## Updates

The app compares its own version with `/api/client/app`. When the panel has a newer installer,
Account › Check for updates (and the check after sign-in) offers it; the app downloads the installer,
starts it with `--update --dir <install>`, and exits. The installer waits for the old
`plugged.host.exe` to close, replaces it, keeps the shortcuts and the Add or remove programs entry,
and relaunches the app. Bump `PLUGGED_APP_VERSION` and copy the new installer to
`PLUGGED_APP_INSTALLER` to publish an update.
