# rux

rux is the panel's moderation and toolbox bot on Discord. Admins have it do things in plain words,
by DM, @mention or "hey rux": warn, time out, ban, run giveaways of credits, set standing rules,
watch channels. Members of the roles they make rux moderator roles have it moderate their server.
rux also checks members' messages by itself: spam without AI, and watched channels and standing
rules with the AI. In `shared/src/rux/`.

rux is the Discord section's **moderation bot**: its switch and token are on Admin › Settings ›
Accounts › Discord ([discord.md](discord.md)), its own settings on Admin › Settings › Accounts ›
rux. Nothing runs until the moderation bot is switched on with a token and rux has a main server
(its own setting, or the Discord section's server).

This page covers rux's core: the settings, who it obeys, how an order runs, the AI, the tables.
The tools and the checks have their own sections below.

## Two ways to run it

| | Without noprofit.space (standalone) | With noprofit.space connected |
|---|---|---|
| Admins | Panel admins (not suspended or frozen) who linked Discord through the Discord section's sign-in provider (Account › OAuth Links) | noprofit.space admins, asked live through noprofit.space's rux routes (or, without them, the account snapshots the panel keeps) |
| Settings | Admin › Settings › Accounts › rux | noprofit.space's (its Admin › rux) when it keeps them: shown read-only on the panel, except the name and the routes' secret |
| AI | The panel's neo providers (Admin › Settings › neo) | The providers noprofit.space runs neo on, while noprofit.space runs neo |
| Credits | The panel's (the tools) | noprofit.space's (the tools) |

noprofit.space's rux routes (`/api/v1/internal/rux/*`: `settings`, `accounts`, and the tools'
payouts) are signed like the panel's other noprofit.space calls. noprofit.space checks them
against its sync secret, so when that is not the panel's noprofit.space API secret, put it in
**Secret of noprofit.space's rux Routes** (shown only while noprofit.space is connected).

While noprofit.space keeps the settings, rux reads them at most once a minute: from memory, then
from an encrypted copy in the cache that every instance shares. A stale copy is used while a new
one is fetched in the background, so a message never waits on noprofit.space; only the very
first fetch (no copy at all) waits up to 5 seconds. A failed fetch is tried again after 30
seconds and rux keeps running on the last copy; with no copy at all the panel's own settings
apply. Values from noprofit.space are clamped into the ranges the panel's page takes. Switching
rux off on noprofit.space stops it.

## Who rux obeys

`access.rs`:

- **admin**: a Discord account linked to an admin (above). Every tool, on every server.
- **moderator**: a member of a role an admin made a rux moderator role. The moderation tools
  only, only on the server of that role, never on staff.
- **staff**: the server's owner, anyone whose roles may kick, ban, time out, manage messages or
  manage the server, and every rux moderator. Standing rules and watched channels leave staff
  alone.

Everything but admin is per server: a moderator of one server is nobody special on another.
Anyone else is ignored before the model is ever called; in DMs they are told so at most once an
hour. "Not allowed" is remembered for a while, until the moderator roles change.

## Orders

An order is a DM to rux, or a message on one of rux's servers that @mentions it, replies to it or
calls it by name ("hey rux, …", "yo rux …", "rux, …"; that needs the Message Content intent).
Bots, webhooks, system messages and servers rux is in but does not work on are ignored. One person
gives at most Orders per Minute orders (`inbox.rs`, which runs in the gateway's event worker, so it
only uses the cache and one insert).

Each order runs in its own task (`agent.rs`):

1. One order at a time per person and channel: an order that comes while the last one still runs
   is answered "still on your last order" and dropped (people resend an order that seems slow, and
   running both would do it twice).
2. The account directory says who wrote it, and the order gets its server: the one it was written
   in, or for a DM the one picked for that DM (`switch_server`, kept a day; the main server until
   then, and for a moderator the first server they moderate).
3. The model gets the system prompt (`prompt.rs`: who rux is, the server, who is talking, the time
   in the settings' zone, the rules of acting), the conversation of the last History minutes, the
   order with the message it replies to and up to its images, and the tools that person may use.
4. The loop: a model turn, its tool calls run, their output fed back (fenced, cut at 6000
   characters), until it answers in text or runs out of Steps per Order. An order ends within ten
   minutes.
5. The answer is posted as a reply to the order, never pinging anyone, in pieces of at most 1900
   characters.

Everything is kept in `rux_messages`, so the next order of the same conversation remembers this
one. Orders that lost their worker (a restart) get a new one after a minute unless the same person
sent another order since; pending orders are given up after ten minutes, running ones after
fifteen.

Rows name people and things by numbers ("case 12", "giveaway 3", "rule 5"), so the model and the
admins can refer to them.

## AI

`ai.rs`: neo's providers on neo's chain (`neo::chain`), in rux's own scope `rux` (its own
cooldowns, daily counts and tool probes), called directly, so no neo limits apply.

- **Orders** try the premium models first (the ones neo opens only to members), then the others.
- **Message checks** (watched channels, standing rules) never reach the premium models: when the
  other models cannot answer, the message is not checked.

The providers are the panel's (Admin › Settings › neo), or, while noprofit.space runs neo (its neo
secret is set), the ones noprofit.space runs neo on (`GET /api/v1/internal/neo/providers`, reused
for a minute and kept as an encrypted last-good copy). The rux page lists both chains in order.
Without any provider the page says rux cannot think, and orders are answered that the AI is
unavailable. A busy AI is waited out for up to two and a half minutes per order.

## The page

Admin › Settings › Accounts › rux (`frontend/src/pages/admin/settings/rux/`), after Discord:

- What stops rux (each reason on its own line), or the moderation bot's state: starting, refused
  (a wrong token, intents not allowed, closed), online without the Message Content intent (DMs and
  @mentions work, "hey rux", standing rules and watched channels do not), online as `<name>`. The
  links that add the bot to each server, with the permissions it needs.
- While noprofit.space keeps the settings: "Set up on noprofit.space", when they were last fetched,
  a link to its Admin › rux, and every field below the name read-only.
- Bot: the name; the main server, time zone, language, orders per minute, steps, history.
- Giveaways and Moderation: the default giveaway channel and mod log of the main server, the
  giveaway limits, the people who may have rux give credits and the most per gift.
- More Discord Servers: up to nine, each with its id, whose server it is, what that host is, its
  giveaway channel, its mod log and its own sites.
- Warnings: the two steps of the warning ladder.
- AI: the chains, the basic guidelines in watched channels, the spam checks and their thresholds,
  the rule cooldown, the host's own sites.
- noprofit.space (only while connected): the routes' secret.

## Settings

`shared/src/settings/rux.rs`, section `rux`:

| Setting | Default | Range |
|---|---|---|
| Name | `rux` | 1 to 32 letters, digits, `-`, `_` |
| Main Discord Server ID | none (the Discord section's) | a Discord id |
| Time Zone | UTC | an IANA zone |
| Language | English | English, German |
| Orders per Minute | 6 | 1 to 120 |
| Steps per Order | 8 | 2 to 30 |
| History | 30 minutes | 1 to 1440 |
| Default Giveaway Channel ID, Mod Log Channel ID | none | a Discord id |
| Most Credits per Winner | 10000 | 1 to 1000000 |
| Most Winners per Giveaway | 25 | 1 to 500 |
| Longest Giveaway | 60 days | 1 to 365 |
| Main Admin (Gives Credits) | none (nobody) | up to 20 Discord user ids |
| Most Credits per Gift | 10000 | 1 to 1000000 |
| More Discord Servers | none | up to 9; each id once, not the main one |
| Timeout from Warning, Timeout Length | 3, 24h | 0 (off) to 100; up to 28d |
| Ban from Warning, Ban Length | 5, 24h | 0 (off) to 100; a duration |
| Watched Channels Also Enforce the Basic Guidelines | on | |
| Catch Spam Automatically | on | |
| Flooding: Messages, Seconds | 7, 10 | 2 to 100, 1 to 600 |
| Repeats: Messages, Seconds | 3, 120 | 2 to 100, 1 to 3600 |
| Mentions, Emoji in One Message | 6, 25 | 2 to 100, 2 to 500 |
| Rule Cooldown | 300 seconds | 0 to 86400 |
| Our Own Sites | none | up to 20 domains (`example.com`) |
| Secret of noprofit.space's rux Routes | none (the API secret) | encrypted, at least 32 characters; empty keeps it |

A payload is checked as a whole before anything changes (`PayloadRux::apply`): a bad id, domain,
zone, language or duration refuses it with the reason (400).

## Tools

`shared/src/rux/tools/`, one folder per group. **M** = moderators get it too (on the servers they
moderate); every other tool is for admins only.

| Group | Tools |
|---|---|
| Moderation (`moderation/`) | `lookup_member` M, `warn_member` M, `timeout_member` M, `kick_member` M, `ban_member` M, `unban_user` M, `list_cases` M, `list_bans` M, `set_nickname` M, `clear_warnings` M, `read_messages` M, `delete_messages` M, `purge_messages` M, `channel_settings` M (slowmode, lock), `role_info` M, `server_overview` M, `manage_role`, `send_message`, `edit_message`, `send_dm` |
| Giveaways (`giveaways/`) | `create_giveaway`, `list_giveaways` M, `edit_giveaway`, `end_giveaway`, `cancel_giveaway`, `reroll_giveaway` |
| Credits (`credits/`) | `give_credits` |
| Standing rules (`rules/`) | `add_rule`, `list_rules` M, `edit_rule`, `remove_rule`, `watch_channel`, `set_moderator_role` |
| Servers (`servers/`) | `switch_server` M (DMs only, with more than one server) |

Everything acts on the order's server only: its members, channels, roles, cases, rules and
giveaways.

### Moderation

- Every action is a **case** in `rux_cases`: who ordered it (or rux itself, with the rule), on
  whom, why, how long, and the message rux acted on. It is posted to the server's mod log (an
  embed in the language of the settings) and carries the order in Discord's audit log. The member
  gets a DM first (warnings, timeouts, kicks and bans), while rux can still reach them.
- **Moderators never act on staff**: not on the owner, people whose roles may moderate, rux
  moderators, panel or noprofit.space admins, themselves or the bot (when the account directory
  cannot say whether someone is an admin, they are left alone). They never touch the mod log
  channel, only read and delete in channels they can see on Discord, and only unlock channels rux
  locked. Admins may do all of it.
- **Warnings** count per server. The warning ladder of the settings times out (from Timeout from
  Warning) or bans (from Ban from Warning) on the warning that reaches it; when Discord refuses that
  step, the warning stays and its case says why.
- **Bans** are for good, or for a while (1 minute to 10 years): the `rux` task lifts a ban whose
  time is up (once a minute) with a case of its own. A new ban replaces a running one.
- Timeouts last up to 28 days, slowmode up to 6 hours. `purge_messages` looks at the last 500
  messages; Discord bulk deletes only the last 14 days, older messages go one by one.
- `manage_role` never hands out a role that comes with rewards (boost roles, the Discord section's
  2x role, roles managed by a bot), nor @everyone. `edit_message` edits only rux's own messages,
  never a giveaway's post.

### Giveaways

- The prize is **always credits**: a title can name the occasion but never promise anything else
  (Plus, tokens, RAM, Nitro, roles…; refused). Without a title it is "500 credits".
- The post has an **Enter** button. Entering needs a Discord account linked to an account
  (noprofit.space's while it is connected, else the panel's: the Discord section's sign-in
  provider); someone who is not linked gets a Link button. The answer only they see comes from a
  task of its own, inside Discord's 3 seconds. Someone already in can leave.
- "Keep out the winners of the last N giveaways" (0 to 10): which giveaways those were is fixed
  when it is created (noprofit.space's last giveaways, or the panel's own), so the draw applies the
  rule the admin read back.
- Limits: the settings' credits per winner, winners and longest giveaway; it ends at least a minute
  from now. A giveaway is refused while nothing can pay it (credits off on the panel, or
  noprofit.space connected without its rux routes).
- **The draw**: the `rux` task (every 10 seconds) or `end_giveaway` moves it from `active` to
  `drawing`, so exactly one run draws. Winners are picked at random (the OS-seeded generator) among
  the entries that have not won, are not kept out, are still linked and still in the server. When
  it cannot be told who is linked, nothing is drawn and it is tried again a minute later. The
  channel is told who won, pinging only them. `reroll_giveaway` draws more winners the same way.
- **Payouts**: with noprofit.space, `POST /api/v1/internal/rux/giveaways` pays each draw under the
  key `rux:calagopus:<giveaway>` (`…:r<n>` for the n-th reroll), so a retry pays nobody twice and
  the keys never meet another panel's on the same noprofit.space; winners who unlinked meanwhile
  are paid there once they link again. Without it, each winner's credits go into the panel's own
  ledger (type `rux_giveaway`, "Giveaway on Discord: <prize>"), each entry once; a winner who is not
  linked (any more) waits. Payouts that are still waiting are tried again, less often the more often
  they failed, for 90 days.
- A draw that died halfway (a restart) goes back to `active` after 10 minutes; the posts' entry
  counts are kept current once a minute.

### Credits

`give_credits` gives credits to one person's account: only the settings' **Main Admin (Gives
Credits)** can order it, and at most Most Credits per Gift at once. With noprofit.space it goes
through `POST /api/v1/internal/rux/credits` as that admin's noprofit.space account (noprofit.space
checks they are an admin there); without it, into the panel's ledger (type `admin`, the reason or
"From <admin> on Discord"). Its key, `rux:grant:calagopus-<hash of the order, server, person and
amount>`, makes an order that is retried give nothing twice (the answer then says so). rux gives
nothing but credits: the credit grant and the giveaways are the only tools that give anything.

### Standing rules and watched channels

- `add_rule` keeps an admin's order ("if anyone asks for new locations, warn them") as a rule:
  what breaks it (`when`), what looks close but does not (`not_when`), its keywords, what rux does
  (warn, timeout, delete, kick, ban; a timeout or ban for a while), whether the message is deleted,
  what the member is told, and in which channels. Keywords are normalized (lowercase, accents off);
  everyday single words (add, new, when, server…) are dropped and at least 3 must be left, at most
  80 are kept. A single word matches whole words only ("ram" not in "program").
- A message that hits a keyword is checked by the AI (the watch, below) against the rule; on a
  clear break rux acts as the rule's author ("rule #5" in the audit log, the rule in the case),
  never on staff or itself, and says it in the channel so the member sees it even with DMs closed.
  `edit_rule` changes only the fields given; `remove_rule` switches a rule off (it stays in the
  table for its cases).
- `watch_channel` has every member message of a channel checked by the AI, against the standing
  rules and the basic guidelines (scams, slurs, harassment, sexual content, advertising; the
  settings switch them). Watched channels are kept by id; a name (`#support`) given elsewhere
  matches the server's text and announcement channels of that name.
- The checks are told facts about the server's host, so its own links and locations are never
  taken for adverts or requests: its sites (Our Own Sites, the panel and status page, the account
  portal, the Discord invite) and, for the panel's own community, its public locations with game
  nodes.
- `set_moderator_role` makes a role a rux moderator role (by default also with Discord's Timeout,
  Kick, Ban and Manage Messages permissions), or takes it back with only the permissions rux added.

### Servers

`switch_server` (DMs only, offered when rux works on more than one server) picks the server a DM
acts on, by id, host name or Discord name (or a unique part of one). The agent checks the move: a
moderator only to a server they moderate; the pick is kept a day.

### Differences from Pelican's tools

- The payout keys on noprofit.space are `rux:calagopus:<id>` (Pelican's are `rux:<id>`), and a
  credit grant's key is made from the order, server, person and amount (`rux:grant:calagopus-…`),
  so a retried order gives nothing twice.
- Without noprofit.space the panel pays itself: credits from the panel's ledger, linked accounts
  from the Discord section's sign-in provider, "the last giveaways" from its own rux giveaways.
- A giveaway is refused when nothing could pay its winners; a draw that cannot tell who is linked
  waits a minute; waiting payouts are tried with growing pauses for 90 days.
- Moderators cannot act on the bot either, and `channel_settings` also keeps them out of the mod
  log and channels they cannot see. A ban for a while lasts at most 10 years.
- Keyword normalizing folds Latin accents and keeps other scripts (Pelican's transliteration
  dropped them).
- `make_image` is not ported: the panel's neo has no image models.

### Checks (tools)

- As an admin: "warn @someone for spam": the DM, the case in the mod log, "list cases" shows it.
  Warn the same member up to the ladder: the timeout follows, with its own case.
- As a moderator (a member of a role made a moderator role): warn a member works; warn an admin,
  another moderator or yourself is refused; `send_message` and giveaways are not offered.
- "Ban @someone for 2 minutes": banned, and unbanned by itself within a few minutes (case "ban
  ended").
- "Giveaway of 100 credits for 2 winners, ends in 3 minutes": the post with Enter. Enter from an
  unlinked account: the Link button; from a linked one: "You are in", press again: Leave. After 3
  minutes: the winners are pinged, their credits are in their accounts (noprofit.space's or the
  panel's credit history). "Reroll it": one more winner, paid too.
- "Giveaway of 1 month of Plus": refused (credits only).
- From the main admin: "give @someone 50 credits": given once; from anyone else: refused.
- "Add a rule: anyone asking for new locations gets warned": `list_rules` shows it with its
  keywords; edit its action to timeout 10m, remove it.
- "Watch #general" then "stop watching #general": both in the mod log.
- With two servers, DM rux "switch to <other server>": the next orders act there.

## Watching channels

TODO: the spam checks, watched channels and standing-rule checks are ported next, into
`shared/src/rux/watch/`.

## The gateway

`shared::rux::register()` plugs rux into `discord::gateway` as the moderation bot. Its intents:
servers, server messages, DMs and Message Content (37377). The gateway hands it events in order on
one worker: an order is stored and started in its own task, every message of rux's servers goes
to the watch (which spawns its checks), and a giveaway's button goes to the giveaways. READY gives
rux its own ids (the bot user and application).

The REST client (`discord.rs`) uses the moderation bot's token: a 429 of up to 10 seconds is
waited out and retried, anything else becomes an error the model can pass on, moderation calls
carry the order in `X-Audit-Log-Reason`, and a path made from what the model wrote is refused when
it could reach another endpoint (`..`, encoded dots or slashes, `?`, `#`).

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/admin/rux/status` | `settings.read` | The settings in effect (noprofit.space's over the panel's), who keeps them and when they were fetched, the servers, what stops rux, whether noprofit.space is connected, the moderation bot's connection, the invite links per server, where the AI comes from and both model chains |

The settings are saved through `PUT /api/admin/settings` with a `rux` payload; while
noprofit.space keeps them the page sends only the name and the secret.

## Tasks

| Task | When | Does |
|---|---|---|
| `rux` | every 10 seconds | Due giveaway draws; once a minute orders that lost their worker, the tool groups' and the watch's ticks; once a day messages older than 30 days are deleted. Nothing while rux does not run |

## Tables

Rows have numbers, not uuids. Discord ids are text. The account columns say who acted: the
noprofit.space account id while it is connected, else the panel user.

- `rux_messages`: rux's short memory: every order (`user`, `pending` until its worker claims it,
  then `running`, `done`, `failed` or `ignored`), the model's answers and each tool's output; the
  rows of one order share its `turn_id`. Deleted after 30 days.
- `rux_cases`: the moderation log: warnings, timeouts, kicks, bans (with their end), unbans,
  cleared warnings, by an order or by rux itself (`automatic`, with the rule).
- `rux_giveaways`, `rux_giveaway_entries`: giveaways of credits and who entered and won.
- `rux_moderator_roles`: the rux moderator roles per server and the Discord permissions rux added
  to them.
- `rux_rules`: standing rules per server.
- `rux_watches`, `rux_watch_items`: watched channels per server and the messages waiting for their
  check.

## Setting it up

1. In the Discord Developer Portal, make an application for rux (or reuse one) and its bot. Under
   Bot, turn on **Message Content Intent**. Copy the bot token.
2. Admin › Settings › Accounts › Discord: switch Discord on with the server id, put the token in
   Moderation Bot and switch it on.
3. Without noprofit.space: pick the Discord sign-in provider there, and link Discord in Account ›
   OAuth Links with an admin account.
4. Admin › Settings › Accounts › rux: the page says what is still missing; add the bot to each
   server with the invite link it shows. Set up neo's providers if rux has none.
5. DM rux "hey" from the linked admin account: it answers.

## Differences from Pelican

- Settings are a settings tab instead of `.env`; read-only while noprofit.space keeps them.
- rux's own token and switch are the Discord section's moderation bot; the gateway is the
  panel's own (no Discord library).
- Panel admins are rux admins only when not suspended or frozen.
- Paths built from what the model wrote are checked before they reach Discord.
- The noprofit.space settings are revalidated in the background, so the gateway never waits on
  noprofit.space once a copy exists.

## Checks

- Admin › Settings › Accounts › rux with Discord off: every reason is listed. Switch on Discord,
  the moderation bot and its token: the list shrinks; with neo's providers set up and (standalone)
  a sign-in provider, the bot's state shows instead.
- With the Message Content intent off in the Developer Portal: "online without the Message Content
  intent" (or refused); turn it on: "rux is online, connected as …".
- The invite links open Discord's add-bot page for each server.
- Save a name with a space: refused. Save a server id with letters, the main server again under
  More Discord Servers, a time zone like `Mars/Base`, a ban length like `soon`: each refused with
  its reason.
- Add a second server, save, reload: it is kept. Remove it: gone.
- With noprofit.space connected: the secret field is there; a secret shorter than 32 characters is
  refused; save one: the field empties and says a secret is stored. With noprofit.space keeping
  the settings: "Set up on noprofit.space", the fields are read-only, the name can still be saved.
- From the linked admin account: DM rux, @mention it, "hey rux": it answers each. From an
  account that is no admin: no answer on the server, "not allowed" once in DMs.
- Send two orders quickly: the second is answered "still on your last order".
- German: switch the panel to Deutsch, the page is fully translated; Language Deutsch makes rux
  post in German.
