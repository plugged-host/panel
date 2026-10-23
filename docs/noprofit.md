# noprofit.space (optional)

[noprofit.space](https://noprofit.space) is the account service behind plugged.host and its sister
services: one account, its plan, credits and linked Discord, shared by every panel. Its accounts
app is not open source; this panel is. The integration is therefore **optional**, and the panel is
complete without it.

## Optional by design

Every feature has a standalone default and, where noprofit.space has something to say, a noprofit
version:

| Feature | Without noprofit.space | With noprofit.space |
|---|---|---|
| Sign-in | Calagopus's own: passwords, two-factor, security keys, OAuth providers, registration | Everyone signs in at noprofit.space |
| Profile, email, password | Edited in the panel | Managed on noprofit.space, mirrored into the panel |
| SFTP passwords | The panel password | The noprofit.space password, checked by noprofit.space |
| Plan, credits, Discord (later phases) | Settings and the panel's own tables | noprofit.space's, synced every minute |

Code that needs one of these asks the settings (`settings.noprofit.connected()`, `.config()`), never
an environment variable, and has a branch for both cases.

## Setting it up

Admin › Settings › noprofit.space:

| Field | Meaning |
|---|---|
| Sign in with noprofit.space | Turns the integration on. Saving refuses when the fields below are incomplete |
| Accounts URL | The accounts app, e.g. `https://accounts.example.com` |
| Sign-in client | The client id the accounts app knows this panel as |
| Sign-in secret | The shared secret the sign-in tokens are signed with, at least 32 characters |
| API secret | The secret of the server-to-server API (account sync, SFTP passwords), at least 32 characters. Without it there is sign-in but no sync |
| Only noprofit.space admins | Lets only noprofit.space admins in, while the panel is not open to everyone |
| Link by email | A first sign-in takes over the panel user with the same email (on by default); off refuses it instead |
| Advanced: audience, callback URL | Defaults `{client}:sso` and `{app url}/api/auth/noprofit/callback` |

The secrets are stored encrypted and never shown again (an empty field keeps the stored one). The
`NOPROFIT_*` environment variables of earlier releases fill the fields in until the settings are
saved once.

Before turning it on, make sure an admin can still get in without it: create an admin with a panel
password (`panel-rs users create --admin true`). Turning noprofit.space off again is refused to
admins who have no panel password, since everyone noprofit.space created has none.

## What changes while it is connected

- **Sign-in**: the login page goes straight to noprofit.space. A sign-in that comes back with an
  error stops on the login page with the reason and *Try again*. *Administrator sign-in*
  (`/auth/login?local=1`) is the password form, for admins with a panel password only.
- **First sign-in**: links the panel user with the account's email (when *Link by email* is on) or
  creates one: no password, email verified, username from the display name, admin when the account
  is a noprofit.space admin.
- **Off while connected**: registration, password resets (except for admins), the panel's own OAuth providers (admin page,
  account links, login buttons) and security keys (account page, login). Their routes answer 404.
  Turned off again, everything is as before.
- **Account sync** (with the API secret): every minute (`sync_noprofit_accounts`), the panel pulls
  the snapshots of its linked accounts and updates admin flag, email, name, avatar, plan, credits
  and Discord. A user whose account noprofit.space no longer has is suspended.
- **SFTP**: users made by noprofit.space have no panel password, so their SFTP password logins are
  checked by noprofit.space (the account's password; 10 wrong guesses per account and minute,
  shared with every panel). SSH keys work as usual.
- **Account page**: a noprofit.space card with the email, plan, credits and Discord, and a link to
  manage them there. Email and password changes are refused for linked users: they live on
  noprofit.space.

## Tables

| Table | What it holds |
|---|---|
| `user_noprofit_accounts` | The link between a panel user and a noprofit.space account, and the last snapshot (email, username, admin, avatar, credits, plan, Discord, when it was deleted) |

## Wire format

What the accounts app must implement to work with this panel (tested against its vectors in
`shared/src/noprofit.rs`):

- **Sign-in**: the panel sends the browser to `{accounts}/sso?client=…&return_to={callback}&state=…`
  and keeps `state` in a cookie and the cache for ten minutes. The accounts app sends it back to
  `{callback}?token=…`.
- **Token**: `base64url(json) "." hex(hmac_sha256(sign-in secret, base64url(json)))`. The JSON has
  `aud` (the audience), `iat`, `exp`, `nonce`, `state`, `account_user_id` and the snapshot fields
  (`email`, `username`, `is_admin`, `avatar`, `credits`, `plan`, `discord`). The panel checks the
  signature, the audience, the lifetime (60 s clock skew) and the state.
- **Server-to-server requests** carry `X-NP-Timestamp`, `X-NP-Nonce` and
  `X-NP-Signature = hex(hmac_sha256(API secret, METHOD "\n" path "\n" timestamp "\n" nonce "\n" hex(sha256(body))))`.
- `POST /api/v1/internal/calagopus/accounts` with `{"account_ids": [...]}` answers
  `{"accounts": [snapshot, ...]}`; an account left out was deleted.
- `POST /api/v1/internal/calagopus/verify-password` with `{"account_id", "password"}` answers
  `{"valid": bool}`; 422 = wrong, 429 = throttled (no verdict).
- Snapshot `plan`: `{cpu, memory, disk, base_*, max_*}` (CPU in %, memory and disk in MiB);
  `discord`: `{id, username, avatar}`.

On noprofit.space's side this panel is a client with its own sign-in secret and API secret, both
equal to the values entered here.
