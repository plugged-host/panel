# Account standing

Suspensions count against the account that owns the server. Every abuse suspension is a **strike**
on its owner, and the strikes give the account's **standing**: the more there are, the longer it
cannot create new servers. It works on its own; [noprofit.space](../noprofit.md), when it is
connected, keeps one record for all its services instead.

## Levels

| Strikes (defaults) | Level | Creating servers |
|---|---|---|
| 0 | `good` | allowed |
| 1 | `warning` | allowed |
| 2 | `limited` | refused for 7 days after the latest strike |
| 3 | `very_limited` | refused for 30 days after the latest strike |
| 4 or more | `suspended` | refused, with no end |

Once a limit's days are over the level stays, but the account may create again; the next strike
starts the days anew. Admins are never limited. The standing changes nothing else: running servers,
starts, resizes and the store work as before.

## What is a strike

An abuse suspension ([abuse.md](abuse.md)): when the checks suspend a server in `suspend` mode, its
owner gets a strike with the reference `server:<uuid>` and the suspension's reason (a devbox's
strike goes to its person, [devboxes.md](devboxes.md)). A reference counts once, however often the
same server is caught. A server that was already suspended, an admin's suspension (kind `manual`)
and noprofit.space's (kind `noprofit`) are no strikes ([suspension.md](suspension.md)).

Dismissing a finding as a false positive in Admin › Abuse takes the panel's own strike for that
server back. noprofit.space keeps a strike it was sent.

Recording a strike never holds up or undoes the suspension: it runs in the background, and a
failure is only logged (`shared/src/standing.rs`, `report_server_strike`).

## On its own

The panel keeps the strikes in `user_strikes` (one row per user and reference, kept when the
server is deleted, deleted with the user) and counts them with Admin › Settings › Security › Abuse
(advanced fields, `shared/src/settings/abuse.rs`):

| Setting | Default | Means |
|---|---|---|
| Limited From (Suspensions) | 2 | strikes from which the account is `limited` (below it, one or more is a `warning`) |
| Limited For (Days) | 7 | days after the latest strike a limited account cannot create servers |
| Very Limited From (Suspensions) | 3 | strikes from which it is `very_limited` |
| Very Limited For (Days) | 30 | days after the latest strike a very limited account cannot create servers |
| Suspended From (Suspensions) | 4 | strikes from which it is `suspended`: no creating at all |

The thresholds must not go down (limited ≤ very limited ≤ suspended); two equal ones skip a level.
The save refuses anything else.

## With noprofit.space

With noprofit.space connected and its API secret set, the panel still writes its own record, and
also sends every strike to noprofit.space, which counts the account's suspensions on all its
services (a VPS suspended on a sister service counts too) and decides the level and its days itself.
The standing of an account noprofit.space made is then noprofit.space's answer, kept a minute per
account in the cache (`standing::account::{id}`), with a link to the account's standing page there.
A user noprofit.space did not make (an emergency admin) keeps the panel's own standing.

- noprofit.space not answering limits nobody: the create goes on and the failure is logged (the
  "no answer" is kept 20 seconds, so an outage does not slow every page down).
- A strike it did not confirm waits in the cache (`standing::strike::server:<uuid>`, a week) and is
  sent again by the account sync every minute while the server stays suspended for abuse. A server
  purged (deleted) before noprofit.space answered is not sent any more; the panel's own record keeps
  it.

The wire format is in [noprofit.md](../noprofit.md#wire-format).

## Where it shows

- **Create server**: an account whose standing refuses creating sees, instead of the form, its level
  and until when (`{level}`, `{date}`), and with noprofit.space a "View your standing" link. The
  create itself refuses with 403 and the same reason (`CreateError::Standing`), so the API and neo
  are refused too; neo's `list_create_options` says so before offering anything.
- **Account page**: once the account has a strike, a **Standing** card with the level, the number of
  suspensions and whether it may create servers (until when), and with noprofit.space a "Details"
  link to its standing page.

## API

| Method | Path | Gives |
|---|---|---|
| `GET` | `/api/client/account/standing` | `{standing}`: `{level, suspensions, can_create, create_blocked_until, url}` (`url` only with noprofit.space), or `null` when the one to ask did not answer |
| `GET` | `/api/client/servers/create` | `options.standing`: the same object while it keeps the account from creating, else `null` |
| `PUT` | `/api/admin/settings` | `abuse.standing_limited_at`, `standing_limited_days`, `standing_very_limited_at`, `standing_very_limited_days`, `standing_suspended_at` (`settings.update`) |

## Check

- Without noprofit.space, abuse mode `suspend`: get a test server suspended for abuse. The owner's
  account page shows the Standing card at Warning, 1 suspension; Create server still works.
- A second one: Limited, and Create server shows the level with the date 7 days after it instead of
  the form; an admin account still creates.
- Dismiss the second finding in Admin › Abuse: the card is back at Warning, and creating works.
- Admin › Settings › Security › Abuse: setting Very Limited From below Limited From is refused.
- With noprofit.space connected: the same suspension reaches noprofit.space as a strike; the card
  and the create page show noprofit.space's level with its link.
