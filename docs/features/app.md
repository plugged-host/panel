# The desktop app (plugged.host App)

plugged.host has a native desktop client for Windows and Linux, `plugged-host/app`, written in Rust (egui). It is not a web
view: the panel's Theme v2 is drawn natively, and the app talks to the same `/api/client` endpoints
the browser does. It signs in with noprofit.space through a loopback listener (the browser hands a
signed token to `127.0.0.1`, the app trades it for a panel session at
`/api/auth/noprofit/app/exchange`).

## In the app

The client's own **App settings** page (More › App settings) shows its version and install folder,
checks `/api/client/app` for a newer installer, installs it in place, and holds the
"check on start" and "open at login" switches, the panel connection and the About links. The app
does not link to its own installer; that page stays in the panel.

## Download page

More › **Download app** appears for **every signed-in user** on a panel where the installer
exists (the public beta). The public settings carry `appDownload: { enabled, version }`; the row
and the page are rendered when `enabled` is true. The page marks the app as a beta and says the
installer is not code-signed yet (Windows SmartScreen asks for More info › Run anyway). Its strings
are `pages.account.downloadApp` and `elements.sidebar.downloadApp`.

| Route | What |
|---|---|
| `GET /api/client/app?platform=` | The version and the asked platform's file (`windows` by default, `linux` for the AppImage, `linux-deb`) as `app` (`enabled: false` without one), and every offered file as `downloads` |
| `GET /api/client/app/download?platform=` | That platform's file, streamed as an attachment (404 without one) |

`PLUGGED_APP_INSTALLER` names the Windows installer on the box, `PLUGGED_APP_LINUX_APPIMAGE` the
Linux AppImage and `PLUGGED_APP_LINUX_DEB` the Debian/Ubuntu package; one that is unset or missing
is not offered, and with none of them the feature is off. The page shows a button per file, the
visitor's own system first. The Windows app asks without `platform`, the Linux app with
`platform=linux`, so each updates from its own file. All three share `PLUGGED_APP_VERSION`. `PLUGGED_APP_RELEASE_NOTES` is shown
on the page. Unset them to take the download away again.

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

## neo

While the panel offers the installer, neo's knowledge has a section on the app (`shared/src/desktop_app.rs`,
`prompt::knowledge_text`): what it is and its version, More › Download app, updates, the SmartScreen
warning, what stays on the website and where its crash log is. Without the installer neo never
mentions it.
