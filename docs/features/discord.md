# Discord

The panel's Discord server as part of the account: players link their Discord account, being in the
server adds a tier to their allowance, every boost level pays credits once, and inviting friends who
stay pays credits. A bot records who joined through whose invite and answers `/invites`,
`/topinvites` and `/setboost`. Subdomains can be kept for players who linked Discord or are in the
server. In `shared/src/discord/`.

Everything is off until Admin › Settings › Accounts › Discord is switched on with a server ID, and
nothing connects to Discord until a bot is switched on there.

## Two ways to run it

| | Without noprofit.space (standalone) | With noprofit.space connected |
|---|---|---|
| The Discord account | The one linked through the OAuth provider the settings name (Account › OAuth Links) | The one on the noprofit.space account (its snapshot) |
| Membership | Checked by the panel: the `sync_discord_members` task, the bot's events, the page's check | noprofit.space's; the panel records it only for the subdomain check |
| Discord tier | Added to the allowance of members | Part of the noprofit.space plan; the panel adds nothing |
| Boosts | Each level paid once (credits on) | noprofit.space's |
| Invites | Counted and paid (credits on) | Counted and shown, not paid: they stay Waiting |
| The page | Linking, membership, tier, invites, boosts, history | Linking on noprofit.space, membership, invites, a link to noprofit.space's Discord page |

"Pays" below means standalone with credits on (Admin › Settings › Economy). Without credits the
page counts invites and boosts and pays nothing.

## The page

Account › Discord Rewards (`frontend/src/pages/dashboard/discord/`) is laid out like the Pelican
panel's Discord rewards tab, in its wording: without a link, a card saying what Discord earns and
where to link (noprofit.space when connected, else Account › OAuth Links), with Join the Discord
server next to it; with one, a card with the credits earned on Discord, the linked name and
whether it is in the server (and what the tier adds), Join or Open the Discord server and Check
for new rewards; then Invite friends (the rules, then Paid, Waiting with the next payout, Left the
server, Did not count, Owed back as full-width rows), Boost the server (the two levels, Received
once paid; with noprofit.space connected, a link to its Discord page in that place) and the
Reward history. At most one filled button: linking, or joining while not in the server. Without
credits (or with noprofit.space) the amounts and the history are left out. The rows are
`.theme-eco-tx` in a `.theme-eco-list` (`app.css`, the Pelican store's rows); the history is the
store's ledger (`elements/economy/TransactionRows.tsx`, Pelican's ledger partial).

## Membership and the tier

`members::record` writes what Discord said about a Discord account to `discord_members`: in the
server or not, boosting since, the boost level. It runs from:

- the `sync_discord_members` task (every five minutes, standalone, with a bot token): first it
  forgets links that are gone (unlinked in Account › OAuth Links, or linked to another account),
  then it asks Discord about 250 linked accounts, the least recently checked first;
- the invite and boost bot's join, leave and member-update events, for linked accounts;
- the page's Check for New Rewards (once a minute per account);
- creating a subdomain, when the owner was not known as a member yet.

Only Discord answering "unknown member" (404 with code 10007 or 10013) counts as having left. A
timeout, a 5xx, a rate limit or a refused token changes nothing, so a Discord outage never takes
anyone's tier away.

The Discord tier (CPU %, memory MiB, disk MiB) is added by `allowance::limits` to the games
allowance of a standalone account whose linked Discord account is in the server
(`discord::tier`), where that allowance has a limit. Losing it records an allowance cut
(`allowance_cuts`, reason `discord`), so the account's servers give back what is over, like any
other smaller allowance ([allowance-cuts.md](allowance-cuts.md)):

- the member left the server (sync, the bot, the page's check);
- the Discord account was unlinked (at the next sync; the allowance itself drops at once);
- an admin lowered the tier: every member who owns a server gets a cut of the difference.

Changing which OAuth provider is the Discord one records no cuts; the next sync forgets the old
links.

## Boosts

Discord says whether a member boosts (`premium_since`), not how often. The boost level is 0 while
not boosting, 2 with the two-boost role (Admin › Settings › Discord › Two-Boost Role ID) or after
`/setboost` set 2, else 1. Each level is paid once per Discord account and once per panel account
(`discord_boost_payouts`, unique on both): level 1 pays One Boost, level 2 pays Two Boosts minus
One Boost (the setting is the total). Stopping and boosting again pays nothing new. A member who
stops boosting loses the two-boost role.

`/setboost user count` (Discord administrators only) sets how often a boosting member boosts: the
two-boost role follows it while they boost, and a linked account is paid the levels it reaches. A
member Discord does not show as boosting stays at 0, and the reply says so.

## Invites

The invite and boost bot keeps every invite's use count. When a member joins it compares the counts
with Discord's (`invites::attribute`): the code whose uses went up is the one they used, and its
inviter gets the row in `discord_invites`. Vanity URLs, widget joins and joins the bot missed have no
inviter and count for nobody; so does a join while the bot could not read the invites (it then
takes Discord's counts as they are and compares from the next join on).

A join is **eligible** when the Discord account was at least Minimum Account Age days old when it
joined (from its id) and it is not the inviter's own. An eligible join **matures** after Hold
hours. The `settle_discord_invites` task (every five minutes, 100 per run) and the page's check pay
the matured joins of linked inviters, after asking Discord whether the member is still there; a
member who left by then pays nothing. Joins of inviters who have not linked yet wait until they
do (and never fill a run).

A member who leaves stops counting. When their invite was paid and Take Back is on, the credits
come back from the inviter's balance (transaction `discord_invite_clawback`); what the balance
cannot cover is owed (`discord_members.invite_debt`) and taken from the inviter's next invite
rewards (`discord_invite_debt`). Coming back counts as a new join: the inviter is the one whose
invite they used now, and a taken-back invite can pay again, a paid one cannot.

The `reconcile_discord_invites` task (hourly, 200 per run) asks Discord about the members still
counted, the least recently seen first, and records the ones it says are gone: leaves the bot
missed while it was offline.

## The bots

`discord::gateway` keeps one gateway connection per bot token, on the primary instance only (the
`discord_gateway` task, every 15 seconds). Three bots, each with its switch, all off by default:

| Bot | Token | Does |
|---|---|---|
| Invite and boost bot | Bot Token | Tracks invites, joins, leaves and member updates; `/invites [user]`, `/topinvites [limit]`, `/setboost user count` |
| Assistant bot | its own | Connects once a handler is registered (`gateway::register(Bot::Assistant, …)`) |
| Moderation bot | its own | rux, the moderation bot ([rux.md](rux.md)) |

Bots that share a token share one connection, their intents added up. A switched-off bot, a
changed token or another server ID closes its connection within 15 seconds. A wrong token or
refused intents stop the bot (tried again after an hour or when its settings change); a refused
Server Members intent reconnects without it, so joins and leaves are missed until it is turned on.
Admin › Settings › Discord shows each bot's state under Bot Status (kept in the cache for 90
seconds, so with several instances it needs Redis).

Slash commands are created or updated one by one on the server (by name), so commands other
software made on the same application stay. `/setboost` is hidden from members without the
Administrator permission, and the panel checks the permission again when it runs. Replies are
embeds in the panel's name, with no emojis; credit amounts only while the panel pays.

The gateway is written on `tokio-tungstenite` (no Discord library): HELLO, IDENTIFY or RESUME,
heartbeats with a missing-ACK check, RECONNECT and INVALID_SESSION, reconnecting with a backoff up
to a minute. A handler is a `gateway::Handler` (intents, commands, events); events run in order on
one worker per connection, so a slow handler never delays the heartbeat.

## Subdomains

Two switches under Subdomains:

- Linked Discord Required: a server's owner needs a linked Discord account to add a subdomain.
- Server Membership Required: the owner's Discord account must be in the server (implies a link;
  needs the bot token, ignored without it).

Checked when a subdomain is created (`subdomains::create`, 403 with the reason). The server's
Subdomains page reads what was recorded (`unavailable` is `discord_link` or `discord_member`, with
the reason shown and adding closed); an owner never checked is shown as allowed and asked about
when they add one. Existing subdomains stay. Renaming and writing records again are not gated.

## Settings

Admin › Settings › Accounts › Discord (`shared/src/settings/discord.rs`):

| Setting | Default | Range |
|---|---|---|
| Discord | off | |
| Server ID | none | a Discord id (15 to 20 digits) |
| Bot Token | none | encrypted; empty keeps it |
| Invite Link | none | `https://`; empty hides every invite link and button |
| Discord Sign-in | none | one of Admin › OAuth Providers; unused with noprofit.space |
| Discord Tier: CPU, Memory, Disk | 0, 0, 0 | 0 to 3200 %, 262144 MiB, 1048576 MiB |
| Credits per Invite | 50 | 0 to 1000000 |
| Minimum Account Age | 30 days | 0 to 3650 |
| Hold | 6 hours | 0 to 720; 0 pays when the member joins |
| Take Back | on | |
| One Boost, Two Boosts (total) | 500, 1250 | 0 to 10000000 |
| Two-Boost Role ID | none | a Discord id |
| Linked Discord Required, Server Membership Required | off, off | |
| Invite and Boost Bot | off | uses the Bot Token |
| Assistant Bot, Moderation Bot | off | each with its own encrypted token |

`/api/settings` has `discord.enabled` (on with a server ID) and `discord.invite_url`; the account
menu shows Discord Rewards while it is on.

## Setting it up

1. In the Discord Developer Portal, make an application (or use the one you sign in with) and its
   bot. Under Bot, turn on **Server Members Intent**. Copy the bot token.
2. Invite the bot to your server with the `bot` and `applications.commands` scopes and the
   permissions **Manage Server** (to read invites) and **Manage Roles** (for the two-boost role).
   Put the bot's role above the two-boost role.
3. Without noprofit.space: add Discord as an OAuth provider (Admin › OAuth Providers: the
   application's client id and secret, Auth URL `https://discord.com/oauth2/authorize`, Token URL
   `https://discord.com/api/oauth2/token`, Info URL `https://discord.com/api/users/@me`, scopes
   `identify` and `email`, Identifier Path `id`, Link Viewable to User and Link Manageable by User
   on; the redirect it shows goes into the application's OAuth2 redirects) and pick it as Discord
   Sign-in.
4. Admin › Settings › Accounts › Discord: switch it on, the server ID, the token, the invite link
   (`https://discord.gg/<your-invite>`), the tier and rewards; switch on the Invite and Boost Bot.
   Bot Status shows "connected as <bot>" within 15 seconds.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/account/discord` | `discord.read` | `{discord}`: source (`panel` or `noprofit`), whether the panel pays, the linked account, membership and when it was checked, the rules, the invite summary, boosts (standalone), credits earned and the last 20 Discord transactions; 404 while Discord is off |
| `POST /api/client/account/discord/check` | `discord.check` | Checks membership and boosts now (with noprofit.space: a fresh snapshot first) and pays the account's matured invites; once a minute (429); returns the page |
| `GET /api/admin/discord/status` | `settings.read` | `{bots}`: each bot's phase (`connecting`, `ready`, `refused`), name, reason, since; `null` when the primary instance stored nothing |

User activity: `account:discord.check`. The user permissions (API key scopes) are the group
`discord`: `discord.read` and `discord.check`.

## Tasks

| Task | When | Does |
|---|---|---|
| `discord_gateway` | every 15 seconds | Starts and stops the bots' connections |
| `sync_discord_members` | every 5 minutes | Membership, boosts and the tier of 250 linked accounts (standalone) |
| `settle_discord_invites` | every 5 minutes, at :30 | Pays matured invites (when the panel pays) |
| `reconcile_discord_invites` | hourly | Finds leaves the bot missed (200 per run) |

## Tables

- `discord_members`: one row per Discord account the panel knows: `discord_id` (key), `user_uuid`
  (the linked account, unique, `NULL` once unlinked), `username`, `avatar`, `in_guild` (counts once
  `checked` is set), `premium_since`, `boosts` (the level), `invite_debt`, `checked`, `created`.
- `discord_invites`: one row per joined member: `inviter_discord_id`, `joined_discord_id`
  (unique), `invite_code`, `account_created`, `joined`, `eligible`, `matures`, `rewarded` +
  `credits`, `departed`, `leave_count`, `rejoin_count`, `revoked` + `revoked_credits`, `last_seen`.
- `discord_boost_payouts`: `discord_id`, `user_uuid`, `level`, `credits`; unique per Discord account
  and level and per panel account and level.

Credits go through `economy::change` with the transaction types `discord_invite`,
`discord_invite_clawback`, `discord_invite_debt` and `discord_boost`.

## Differences from Pelican

- One Discord account per panel account through an OAuth provider instead of the Discord plugin's
  own OAuth flow; noprofit.space's when connected.
- The boost level comes from the two-boost role and `/setboost` instead of guessing from the
  server's boost count.
- A matured invite is checked with Discord right before it is paid.
- Giveaways, the legacy invite baselines and the restore of false departures are not ported.
- Settings are a settings tab instead of `.env`; the bots are switches, off by default.

## Checks

- Admin › Settings › Accounts › Discord: switch on with a server ID; the account menu shows
  Discord Rewards. A server ID with letters, or an invite link without `https://`, is refused.
- Save a token: the field empties and says a token is stored; saving again keeps it.
- Switch on the Invite and Boost Bot: Bot Status shows "connected as …"; `/invites`, `/topinvites`
  and `/setboost` appear in the server. A member without Administrator does not see `/setboost`.
- Turn off Server Members Intent in the Developer Portal and reconnect: the status says joins and
  leaves are missed. A wrong token: refused.
- Standalone: link Discord in Account › OAuth Links; Discord Rewards shows the account and, after
  Check for New Rewards, "In the server" and the tier. Check again within a minute: refused.
- Set a tier: a member's allowance grows by it (create a server with it). Leave the server and
  check: the allowance drops and an allowance cut is recorded.
- Boost the server with a linked account: One Boost is paid once (Discord Rewards › Reward
  history). `/setboost user 2`: the role is given and the second level paid; `/setboost user 0`
  takes the role back. `/setboost` on a member who does not boost: the reply says Discord shows no
  boost, nothing is paid.
- Invite a second account (older than the minimum age) with Hold 0: the inviter has the credits
  right away (with a hold: after the next settle or the inviter's check once it is over). The
  invited account leaves: the credits are taken back; with a balance of 0 they show as Owed Back
  and come off the next invite.
- `/invites` and `/topinvites` show the counts; with credits off no credit amounts.
- Subdomains: switch on Linked Discord Required; an owner without a link sees the reason on the
  Subdomains page and cannot add one. Server Membership Required: an owner who left cannot add one.
- With noprofit.space connected: the page links to noprofit.space for linking, boosts and the tier
  are gone from it, invites are counted but stay Waiting, no credits are paid.
- Clear the invite link: no Join or Open button anywhere.
- Switch Discord off: the account menu entry is gone, the routes answer 404, the bots disconnect.
