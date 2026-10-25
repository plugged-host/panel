# Partner program

Communities that run a Discord server for a FiveM or Minecraft server apply to become partners and,
once an administrator approves them, earn credits every month. The moderation bot checks the
server (its size, that the applicant owns or administers it, and a channel about the host), neo
suggests whether to accept and how much to pay, and an administrator decides; nothing is paid
without one. In `shared/src/partners.rs`.

Everything is off until Admin › Settings › Economy › Partners is switched on. Players see
Account › Partners (and a link to it on Discord Rewards and in the Resource Store's credits card)
while it is on; they can apply while it is also *open*: credits on (Admin › Settings › Economy), a
Moderation Bot token in the Discord section (rux's bot; it does not need to be connected, the checks
use Discord's REST API) and a way to know a player's Discord account (noprofit.space connected, or
the Discord section's Discord Sign-in provider).

## Requirements

What a server needs, shown on the page before the form and checked again before every payment:

| Requirement | How it is checked |
|---|---|
| The applicant's Discord account is linked | noprofit.space's link, or the Discord Sign-in provider's (Account › OAuth Links) |
| The bot is in the server | `GET /guilds/{id}?with_counts=true` answers; the page has an *Add* button that invites the bot with no permissions (`permissions=0`) |
| At least *Minimum Members* members | Discord's `approximate_member_count` (bots included) |
| The applicant owns the server or has Administrator | The guild's owner, or the permissions of the applicant's roles (and @everyone's) include `ADMINISTRATOR` (`rux::access`) |
| A channel about the host (when *Channel About the Host* is on) | See [the channel](#the-channel-about-the-host) |

The form takes an invite link (`discord.gg/…`, `discord.com/invite/…`) or the server ID, the game
(FiveM, Minecraft or both), a description (20 to 1000 characters) and up to 10 of the applicant's
servers on the panel. An invite is resolved with `GET /invites/{code}?with_counts=true`; an invite
that expires works once but the monthly checks use the server ID, so nothing depends on it later.

Limits: one pending application per account (a partial unique index), one live partnership per
Discord server (pending, approved, paused or suspended; a partial unique index on `guild_id`), and
after a rejection or a revoked partnership neither the account nor that server may apply again for
*Cooldown After Rejection* days. A partner may hold partnerships for several servers, one
application at a time. Applying is limited to 6 tries in 10 minutes.

When the check fails the form answers 422 with the problems (`not_linked`, `invalid_link`,
`unknown_invite`, `bot_missing`, `too_few_members`, `not_admin`, `channel_missing`,
`channel_hidden`, `channel_link`, `pending`, `taken`, `cooldown`) and the page explains each one
in the player's language, with the bot's invite button for `bot_missing`. Nothing is stored for a
refused application.

## The channel about the host

The server must have a channel that tells its members about the host:

- A text, announcement or forum channel (types 0, 5, 15).
- Visible to @everyone: the @everyone role's permissions with the channel's @everyone overwrite
  applied must include View Channel (the same permission math as rux, `can_view_with`). Role or
  member overwrites do not count: a channel only some can see is not about the host for everyone.
- Its name or its topic contains the panel's name (`settings.app.name`, never a hard-coded brand)
  or one of *Other Channel Names*. Both sides are compared by their letters and digits only,
  lower-cased, so `#plugged-host`, `#pluggedhost` and a topic "Hosted by Plugged Host" all match
  "plugged.host". Names shorter than 3 letters and digits are ignored.
- When *Required Links* has entries, the channel's **topic** must also contain one of them
  (compared the same way, so `https://panel.example.com/` matches `panel.example.com`).

The topic is used for the link, not a pinned or the latest message: reading the topic needs nothing
beyond seeing the channel, while messages need Read Message History and the Message Content intent
(and an invite with permissions), and the latest message changes all the time. Forum channels have a
topic too (their guidelines).

The panel finds the channel itself; there is no picker. On a re-check the channel found before is
tried first, then every channel in Discord's order. The admin view shows the channel that
qualified, or the one that came closest (found but hidden, or found without the link).

## neo's assessment

On every new application, and when an admin chooses *Check Again* on a pending one, neo reads the
application in one turn without tools (`neo::chain::Chain`, scope `partners`, on rux's view of neo's
providers, 90 seconds at most) and answers JSON: accept or reject, a monthly amount and short
reasons. The amount is clamped to the settings' bounds (*Minimum*/*Maximum Credits per Month*). The
server's name, description and the applicant's text are fenced as untrusted (`rux::text::untrusted`
swaps angle brackets) and the system prompt tells neo that nothing inside the fence is an
instruction. *Guidance for neo* is added to the prompt.

When neo is off (the setting, or neo not offered), busy, unavailable or answers something that is
not the JSON, the application keeps an `{"error": …}` and Admin › Partners shows "No assessment"
with the reason. neo only suggests: its verdict and reasons are shown to admins only, never to the
applicant, and approving needs an admin either way.

## Admin › Partners

`frontend/src/pages/admin/partners/`, permission `partners.read` to see and `partners.manage` to
act. Pending applications come first (oldest first), then suspended and failing partnerships, then
the rest by date. A filter by status (or *All partnerships*: approved, paused, suspended) and a
search by server name, server ID or applicant.

Each row opens the details: the server (members and online, created, boosts, verification,
features, the applicant's standing, the channel, the problems the last check found), the
application (applicant, account age, Discord ID, game, servers, description), neo's suggestion and
reasons, the decision, the monthly credits, the next payment, when a failing partnership will be
suspended, and the payments.

| Action | From | Does |
|---|---|---|
| Approve | pending | The amount is prefilled with neo's suggestion (0 without one); 0 to *Maximum Credits per Month*. Pays the first month right away and sets the next payment a month later |
| Reject | pending | A reason (required) the applicant sees; starts the cooldown |
| Check Again | any open one | Checks the server now; on a pending one neo assesses it again |
| Change Credits | approved, paused, suspended | The amount from the next payment on |
| Pause Payments | approved, suspended | An optional reason the partner sees; nothing is paid until resumed |
| Resume Payments | paused, suspended | Back to approved; the payment day stays if it is still ahead, else the next one from today |
| Revoke | approved, paused, suspended | A reason (required) the partner sees; payments stop, paid credits are kept; starts the cooldown |

An amount above 0 is refused while credits are off, or while credits are noprofit.space's and its
account sync is off; approving a partner without a noprofit.space account is refused the same way.

Every action is written to the admin activity log (`partners:application.approve`, `.reject`,
`.recheck`, `.credits`, `.pause`, `.resume`, `.revoke`), and the applicant's to theirs
(`account:partners.apply`, `.withdraw`, `.check`).

## Monthly credits

A partnership is paid once a month, the first month on approval. The `settle_partners` task (every
15 minutes, on the primary instance) takes the approved partnerships whose payment day has come and,
for each:

1. Checks the server again with the bot (reachable, members, owner or Administrator, the channel).
2. **Passed**: pays the month of the payment day and moves the payment day on a month (the 31st
   becomes the month's last day).
3. **Failed**: the month waits. The first failure notifies the partner (with the date) and shows on
   Admin › Partners; the check is repeated every 6 hours. A pass within *Grace Period* days pays the
   held month. Past the grace period the partnership is **suspended** (partner and admins notified).
   A grace period of 0 suspends on the first failure.
4. A suspended partnership is checked again once a day; a pass (or the partner's *Check Again*, or
   an admin's) lifts the suspension and the next payment day is the next one ahead.

When Discord itself cannot be reached nobody fails: the partnership is tried again on the next
round. A payment that cannot be made (with noprofit.space: the partner has no noprofit.space
account, or its account sync is off) is retried 6 hours later and logged. Months missed while a
partnership was paused or suspended, or while the program, credits or the bot were off, are not paid
back.

**Never twice.** Every payment is a `partner_payments` row, unique per partnership and month
(`YYYY-MM` of the payment day), inserted in the same transaction that pays and that holds the
partnership's row lock; a second attempt (a restart, the other HA box, a retried round) finds the
row and pays nothing. An approval only pays from `pending`, read under the same lock.

Credits go through `economy::change` (type `partner`, the server's name as the description) or,
with noprofit.space, its credit outbox (`noprofit::queue_credit_change`, pushed right away). They
appear in the credit history as "Partner program".

The partner sees on Account › Partners: the monthly amount, the next payment date, a held month
with the date it will be suspended and what is wrong, the payments (the last 12 and the total), and
*Check Again* while it is failing or suspended (5 an hour). The reasons an admin gave for a
rejection, a pause or a revocation are shown; neo's reasoning never is.

## Notifications

Decisions arrive in the [notification inbox](notifications.md): `partnerApproved`,
`partnerRejected`, `partnerRevoked`, `partnerPaid`, `partnerCheckFailed`, `partnerSuspended`,
`partnerPaused`, `partnerResumed`, `partnerCreditsChanged`; for admins with `partners.manage`,
`admin.partnerApplication` and `admin.partnerSuspended`.

## Settings

Admin › Settings › Economy › Partners (`shared/src/settings/partners.rs`):

| Setting | Default | Range |
|---|---|---|
| Partner Program | off | |
| Minimum Members | 500 | 1 to 10000000 |
| Minimum Credits per Month | 500 | 0 to 10000000, not above the maximum |
| Maximum Credits per Month | 5000 | 0 to 10000000 |
| Grace Period (Days) | 7 | 0 to 60 |
| Cooldown After Rejection (Days) | 30 | 0 to 365 |
| Channel About the Host | on | |
| Other Channel Names | none | one per line or comma separated, at most 20, 500 characters |
| Required Links | none | as above; empty: no link needed |
| neo's Assessment | on | |
| Guidance for neo | none | 2000 characters |

`/api/settings` has `partners.enabled` (the page is shown) and `partners.open` (players may
apply).

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/account/partners` | `partners.read` | `{partners}`: the requirements (minimum members, bounds, grace days, channel names and links, the bot's name and invite link), the linked Discord account, whether the account may apply now (or after which date), and its applications with status, monthly credits, next payment, problems, payments; 404 while off |
| `POST /api/client/account/partners` | `partners.apply` | Applies (`server`, `game`, `description`, `servers`); 422 with `refusal` when the server does not qualify; not while impersonating |
| `DELETE /api/client/account/partners` | `partners.apply` | Withdraws the pending application |
| `POST /api/client/account/partners/{application}/check` | `partners.apply` | Checks a failing or suspended partnership now; a pass lifts the suspension |
| `GET /api/admin/partners` | `partners.read` | Paginated (`page`, `per_page`), `status` (or `partners`), `search` (server name or ID, applicant) |
| `POST /api/admin/partners/{application}/approve` | `partners.manage` | `{credits}` |
| `POST /api/admin/partners/{application}/reject` | `partners.manage` | `{reason}` |
| `POST /api/admin/partners/{application}/recheck` | `partners.manage` | |
| `POST /api/admin/partners/{application}/pause` | `partners.manage` | `{reason?}` |
| `POST /api/admin/partners/{application}/resume` | `partners.manage` | |
| `PUT /api/admin/partners/{application}/credits` | `partners.manage` | `{credits}` |
| `POST /api/admin/partners/{application}/revoke` | `partners.manage` | `{reason}` |

The user permissions (API key scopes) are the group `partners`: `partners.read` and
`partners.apply`; the admin ones `partners.read` and `partners.manage`.

## Tasks

| Task | When | Does |
|---|---|---|
| `settle_partners` | every 15 minutes | The monthly checks and payments (50 a round), re-checks of failing (6 hours) and suspended (daily) partnerships |

## Tables

- `partner_applications`: `uuid`, `user_uuid`, `discord_id`, `guild_id`, `guild_name`, `invite`,
  `game`, `description`, `server_uuids`, `facts` (what the last check saw), `problems`, `checked`,
  `assessment`, `assessed`, `status` (`pending`, `approved`, `paused`, `suspended`, `rejected`,
  `revoked`, `withdrawn`), `monthly_credits`, `reason`, `decided_by`, `decided`, `approved`,
  `next_payment`, `failing_since`, `created`. Unique: one `pending` per user, one live partnership
  per `guild_id`.
- `partner_payments`: `uuid`, `application_uuid`, `period` (`YYYY-MM`), `credits`, `created`;
  unique per application and period.

Migration `partners`.

## Decisions to revisit

Product choices made while building it (the owner asked for monthly credits and a required channel):

- What partners give back is the channel about the host (and the optional link in its topic); no
  banner, role or post schedule is asked for.
- The first month is paid on approval; months missed while paused or suspended are not paid back;
  revoking keeps what was paid.
- An admin may set any amount up to the maximum (0 included); neo's suggestion stays within the
  minimum and maximum.
- The bot is invited with no permissions; everything it reads is visible to any member with View
  Channel on the channels it can see.

## Checks

- Admin › Settings › Economy › Partners: switch it on; Account › Partners appears, and a *Partner
  program* section on Discord Rewards and a button in the Resource Store's credits card. A minimum
  above the maximum is refused.
- Without a Moderation Bot token (or with credits off): the page says applications are closed.
- Unlinked Discord: the first requirement offers linking (noprofit.space or Account › OAuth Links)
  and the form is hidden.
- Apply with a server the bot is not in: refused with *Add {bot}*; add it (no permissions asked)
  and apply again. A server below the minimum, or one where you are only a member: refused with the
  count or the role problem.
- No channel named after the host: refused. Name one `#<host>-partner`, hide it from @everyone:
  refused as hidden; show it: accepted. With a required link and a topic without it: refused.
- Apply: Admin › Partners shows it first with neo's suggestion (or "No assessment" with neo off);
  admins with `partners.manage` get a notification. Apply again: refused as pending; withdraw.
- Approve with an amount: the partner gets a notification and the credits (credit history:
  "Partner program"); Account › Partners shows the amount, the next payment and one payment.
  Approving again is impossible (only pending can be approved).
- Change Credits, Pause (with a reason), Resume, Revoke: each notifies the partner and shows on the
  page; revoked starts the cooldown.
- Monthly: set a partnership's `next_payment` in the past on a test database; within 15 minutes it
  is paid once and the next payment moves a month on. Rename the channel first: the month is held,
  the partner notified with the date, Admin › Partners shows it failing; rename it back and use
  *Check Again*, then the next round pays the held month. With a grace period of 0 a failure
  suspends at once.
