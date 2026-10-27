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
| Credits and the Resource Store | The panel's own (`users.credits`, its own prices) | noprofit.space's balance, store and prices ([features/economy.md](features/economy.md)); credits the panel gives or takes (Discord invites, admins) are sent to it |
| Deleting an account | Admins delete users | Deleting the noprofit.space account deletes the panel user and its servers (asked by noprofit.space, or by the sync once the account is gone) |
| Plan, Discord (later phases) | Settings and the panel's own tables | noprofit.space's, synced every minute |
| Account standing | The panel's own strikes, counted with Admin › Settings › Security › Abuse | noprofit.space's, counted across all its services ([features/standing.md](features/standing.md)) |

Code that needs one of these asks the settings (`settings.noprofit.connected()`, `.config()`), never
an environment variable, and has a branch for both cases.

## Setting it up

Admin › Settings › Accounts › noprofit.space:

| Field | Meaning |
|---|---|
| Sign in with noprofit.space | Turns the integration on. Saving refuses when the fields below are incomplete |
| Accounts URL | The accounts app, e.g. `https://accounts.example.com` |
| Sign-in client | The client id the accounts app knows this panel as |
| Sign-in secret | The shared secret the sign-in tokens are signed with, at least 32 characters |
| API secret | The secret of the server-to-server API (account sync, SFTP passwords), at least 32 characters. Without it there is sign-in but no sync |
| Only noprofit.space admins | Lets only noprofit.space admins in, while the panel is not open to everyone |
| Link by email | A first sign-in takes over the panel user with the same email (on by default); off refuses it instead |
| Delete gone accounts after (days) | How long a user (and its servers) whose account the sync found missing stays suspended before it is deleted here too; 7 by default, 0 never deletes |
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
- **Account sync** (with the API secret): every minute (`sync_noprofit_accounts`), the panel first
  sends its credit changes, then pulls the snapshots of its linked accounts and updates admin flag,
  email, name, avatar, plan, credits and Discord. A smaller plan shrinks the account's servers that
  are then over it ([features/allowance-cuts.md](features/allowance-cuts.md)).
- **Accounts gone from noprofit.space** (left out of its answer, without the deletion reaching the
  panel): the user is suspended and its servers too, with the deletion time of *Delete gone
  accounts after* ([features/suspension.md](features/suspension.md)). The sync keeps asking for
  them: an account that is back lifts both suspensions (a user an admin had suspended stays so).
  After the days, the user is deleted like below, ten per run; one that cannot be deleted is tried
  again later.
- **Deleting an account on noprofit.space** asks the panel first (`/api/noprofit/account/summary`
  for the confirmation page, then `/delete`): the user's servers are deleted on their nodes with
  all their backups, then the backups still kept of servers deleted before, the activity that names
  the user (it carries IP addresses) and the user. The panel refuses (and noprofit.space keeps the
  account) for its last admin, or when a node cannot delete a server; servers deleted before that
  stay deleted, and asking again carries on.
- **Credits made here** (Discord invite rewards and their clawbacks, an admin's change) go to the
  account's balance on noprofit.space: written to `user_noprofit_credit_changes` with the change,
  sent right after and by every sync until noprofit.space answers. Each is applied there once
  (its id is the key), a deduction at most down to zero; one noprofit.space refuses for good (no
  such account) is kept as applying nothing. Until noprofit.space confirms, the balance a clawback
  or an admin's deduction counts on is the last snapshot plus the changes not sent yet.
- **SFTP**: users made by noprofit.space have no panel password, so their SFTP password logins are
  checked by noprofit.space (the account's password; 10 wrong guesses per account and minute,
  shared with every panel). SSH keys work as usual.
- **Account page**: a noprofit.space card with the email, plan, credits and Discord, and a link to
  manage them there. Email and password changes are refused for linked users: they live on
  noprofit.space.
- **Subuser invites** reach existing panel users only: an email no panel user has is refused with
  "no *app* account uses that email yet: ask them to sign up first, then invite them" instead of
  making a password-less user and mailing it (`server_subuser.rs`, as the Pelican panel's
  `GuardedSubuserCreationService`). Without noprofit.space, upstream's invite makes the user.
- **Admin › Users**: no Create (users come from sign-ins; emergency admins from the `users create`
  command), and a linked user's username and email cannot be changed there
  ([features/admin-users.md](features/admin-users.md#identity-lock-noprofitspace)).

## Tables

| Table | What it holds |
|---|---|
| `user_noprofit_accounts` | The link between a panel user and a noprofit.space account, and the last snapshot (email, username, admin, avatar, credits, plan, Discord, when it was deleted, whether the sync suspended the user then) |
| `user_noprofit_credit_changes` | Credit changes made here for noprofit.space: amount, type, description, when it was sent, what it applied and the balance after, attempts and the last error |

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
- `POST /api/v1/internal/calagopus/accounts` with `{"account_ids": [...], "usage": [{account_id,
  user_uuid, cpu, memory, disk, servers}, ...]}` answers `{"accounts": [snapshot, ...]}`; an account
  left out was deleted. `usage` is what each account's servers here use (`noprofit::Usage`):
  noprofit.space keeps it as the service `plugged-host-calagopus` in the account's pool, so its
  other services (switched.cloud, the Pelican panel) have that much less, as this panel has less
  for theirs.
- `POST /api/v1/internal/calagopus/verify-password` with `{"account_id", "password"}` answers
  `{"valid": bool}`; 422 = wrong, 429 = throttled (no verdict).
- Snapshot `plan` (accounts `planFor`): `{cpu, memory, disk, base_*, max_*, total_*, elsewhere_*,
  cut_*}` (CPU in %, memory and disk in MiB). `cpu`/`memory`/`disk` are this panel's share of the
  pool (what is enforced), `total_*` the whole pool, `elsewhere_*` what the other services use of
  it, `cut_*` what lower bases took from this panel so far (only counting up; their growth is what
  the servers give back, [allowance cuts](features/allowance-cuts.md)). The sidebar, the store and
  Create Server show the whole pool with the other services' part counted as used, like the
  Pelican panel; `discord`: `{id, username, avatar}`.
- `POST /api/v1/internal/calagopus/store` with `{"account_id", "discount"?}` answers the account's Resource Store:
  `{credits, catalog: [{resource, step, total, limit, soft_limit, offers: [{steps, amount, price, full_price}]}],
  transactions: [{amount, balance_after, type, description, created_at}], discount}` (the last 10).
  `discount` is the panel's sale in percent (0 to 100, sent only while one runs): every `price` is
  `full_price` with it taken off, rounded half up, and the answer's `discount` says what was
  applied ([sales](features/economy.md#sales)). Optional
  `discord: {bonus, max_bonus, credits_per_invite, credits_per_boost}` (`bonus` and `max_bonus` as
  `{cpu, memory, disk}`, accounts `DiscordGuilds::status`: what the Discord servers the account is in
  add, and what all of them would) lets the store offer what is left of the Discord rewards; without
  it the store offers none.
- `POST /api/v1/internal/calagopus/purchase` with `{"account_id", "resource", "steps", "price",
  "discount"?}` buys at that price (the discounted one while a sale runs): 200 `{ok, purchase, ...store}`; 409 `{error: "price_changed", ...store}` when
  the price moved; 422 `{error: "refused", message}` (not for sale, balance short).
- `POST /api/v1/internal/calagopus/credits/spend` with `{"account_id", "amount", "type",
  "description", "idempotency_key"}` takes credits for something bought on the panel (the second
  membership): 200 `{credits}` (the balance after; a repeated key changes nothing), 422 when the
  balance is short, 409 when the key belongs to a different spend.
- `POST /api/v1/internal/calagopus/credits/apply` with `{"account_id", "user_uuid", "delta",
  "type", "description", "idempotency_key"}` gives (or, negative, takes) credits made on the panel:
  200 `{applied, credits}` (what was applied, a deduction at most the balance; for a repeated key
  what was applied the first time; `credits` is the balance now), 404 for an unknown account, 409
  when the key belongs to a different change, 422 for an invalid one. The panel retries anything
  else.
- `POST /api/v1/internal/calagopus/standing` with `{"account_id"}` answers the account's standing:
  `{level, suspensions, can_create, create_blocked_until, url}` (`level` one of `good`, `warning`,
  `limited`, `very_limited`, `suspended`; `create_blocked_until` ISO 8601 or null; `url` the
  account's standing page). The panel waits 4 seconds for it and limits nobody without an answer
  ([features/standing.md](features/standing.md)).
- `POST /api/v1/internal/calagopus/standing/strikes` with `{"account_id", "reference", "reason"}`
  (`reference` = `server:<uuid>`) records an abuse suspension made here: 201 with the standing
  after it, 200 when the reference was recorded before (it counts once). 404 and 422 are refused
  for good; the panel sends anything else again with the account sync.

noprofit.space calls the panel too, signed the same way with the API secret (a minute's tolerance,
each nonce once; 401 `unauthorized` otherwise, 60 requests per minute and address):

- `POST /api/noprofit/account/summary` with `{"account_id"}` answers `{"found": false}` or
  `{"found": true, "username", "servers": [names]}`: what deleting the account deletes here.
- `POST /api/noprofit/account/delete` with `{"account_id"}` deletes the panel user of the account
  (above): `{"deleted": true, "servers": n}` (also when no user has it), or 409 `{"error":
  "refused", "message"}` with the reason to show. A deletion carries on when the request is cut
  off.

On noprofit.space's side this panel is a client with its own sign-in secret and API secret, both
equal to the values entered here.
