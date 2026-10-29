# neo

neo is the assistant in the panel's chat: a model that reads a server's console and files and acts
on them with the person's own permissions, asking before it changes anything unless the person
allowed it to go ahead. The chat runs on the account (account questions, the store, servers in
general) and on each server (that server). Its code is in `shared/src/neo/`; the models it talks
to (provider slots, the noprofit.space gateway, the usage windows) are described under
[Settings](#settings) below.

## Who can use it

- The admin turns it on (Admin › Settings › neo, `neo.enabled`) and gives it a model: provider
  slots of its own, or noprofit.space's gateway when noprofit.space is connected and runs neo. The
  public settings say `neo.enabled` only when both are true (`neo::offered`), and the chat's
  launcher is hidden otherwise.
- The person needs the account permission `neo.chat` (API keys can be scoped without it). On a
  server they also need to own it or be a subuser with `neo.access`; an admin who is neither does
  not get neo on someone else's server (`tools::may_use_on`).
- Each person has their own switches (`user_neo_settings`): neo on or off for them (on by
  default), making changes without asking (off by default), their model, and the moment they
  agreed to neo's terms. Until they agree, the chat shows the introduction instead.

Every step checks these again (`Agent::blocked`): `disabled`, `turned_off`, `consent`,
`no_access`, `not_yours`.

## Conversations

A place (the account, or one server) has one open conversation per person
(`neo_conversations.archived IS NULL`), created when the chat first opens. "New chat" puts it into
the history; reopening an old chat puts the open one there (an empty one is deleted instead). The
history lists a place's 50 latest chats with a message. Chats untouched for
`neo.retention_days` days (30) are deleted by the `prune_neo_conversations` task, every night at
04:45; a person can delete one chat, or all of theirs on every server at once.

`neo_messages` holds every row in order (`seq`): the person's messages, the model's answers (with
the tool calls it made), one row per tool call with its status and result, and notices (limits,
errors) that only the chat shows. Images are stored in `neo_images` as JPEG (at most 4 per message,
1568 px on the longest side, 12 megapixels before shrinking); the chat loads them through the
account's images route, which only serves the person's own.

## The loop

`shared/src/neo/agent.rs` does one unit of work per request, so no request holds a worker for a
whole answer and a turn carries on after the person leaves the page:

1. **Send** stores the message (and images), answers anything still waiting from the previous
   request as skipped ("Not run: the user sent a new message instead."), names the chat after its
   first message and marks it running. At most `neo.messages_per_minute` per person.
2. **Step** takes the conversation's lock (one step at a time per chat; another step gets `busy`)
   and then either runs the next queued or approved tool call, or, with none left, marks the chat
   as waiting for approval, or, with nothing waiting, calls the model once. The answer streams to
   the chat while it is written. At most `neo.tools_per_minute` tool calls per person.
3. The chat steps again while the result is `continue` (or after `retry_after` seconds when
   `busy`), and stops on `done`, `awaiting_approval`, `limited`, `unlinked`, `error`, `blocked` or
   `redirect` (a start gate took a start over; the person is taken to its page).

After `neo.max_steps` model calls in one request, the next call is asked for a final answer without
tools; calls it still makes are stored as skipped and the turn ends.

What the model sees (`shared/src/neo/context.rs`): the system prompt, then the conversation from
the newest message back as far as `neo.context_tokens` allows. Older tool output is shortened to a
stub (the 3 most recent calls stay whole); tool output is always fenced as untrusted data
(`<tool_output untrusted="true">`), so text in a file or the console cannot pose as instructions.
Images reach only models that can see them; the others get a placeholder line.

## The prompt

`shared/src/neo/prompt.rs` writes what the model is told. Everything comes from the settings and the
database, and a feature that is off is never mentioned. The prompt names no host, service or link the
settings do not give; noprofit.space is named only while it is connected.

- **Panel chat** (`prompt::system`, called by the agent loop before every model call):
  1. who neo is and how it works;
  2. what the host is and how it works (below);
  3. the rules (tool output and the server name are untrusted data, masked secrets, act instead of
     asking, never spend or delete unless asked, smallest safe change, reply in the user's language);
  4. last, "This conversation": the user, the server (name flattened to one line, at most 100
     characters, JSON-quoted; short id; the game's catalog name; the node), its limits, the user's
     permissions on it (the owner's `*` reads "all (server owner)") and the approval mode.

  Everything before "This conversation" changes only with the settings and the account's
  noprofit.space plan, so providers that cache prompt prefixes reuse it between turns. "This
  conversation" also gives the account's membership (or "no membership") with its end and the
  token balance (`prompt::account_state`, read every turn), so neo only offers what the user does
  not have yet.
- **Offers** (`knowledge_text`): the knowledge lists what else tokens buy while it is sold (the
  waiting-room day pass, a server's keep-awake pass and boost ([server upgrades](server-upgrades.md)),
  one more day of neo usage, each with its price and page), and one short rule: when the user asks
  about idle stops, the start queue, the waiting room, ads, limits or running out of resources
  (only the topics whose feature is on), neo mentions the one matching offer once, with its link (a
  membership, the Resource Store, the day pass, the keep-awake pass, the boost, extra neo usage),
  and never pushes it. Tokens never turn into credits, so neo never offers that.
- **What neo knows** (`prompt::knowledge`, part of both prompts): accounts and support, creating
  servers and the allowance, locations, credits, tokens and memberships, starting and idle servers,
  backups, the Minecraft log check, subdomains, FiveM, Minecraft and VS Code. Each comes from its
  settings:

| Topic | Read from |
|---|---|
| Name, URL, language, release | `app::*`, `shared::RELEASE` |
| Where accounts live, terms, the account's plan limits | `noprofit` settings (connected or not), `user_noprofit_accounts.plan` |
| Allowance, plans, server limits, resizing, deleting, moves | `games::*`, public `plans` |
| Location tiers | `locations.tier` of public locations |
| Credits, store prices, tokens, memberships, burst cores | `economy::*`, the earn providers the Tokens page offers |
| Start queue, waiting room, in-game ads | `queue::*`, `ads::*` |
| Idle stop and parking | `limbo::*` |
| Backups, automatic failover | `recovery::*` |
| Subdomains | `subdomains::*` (active = on with a token) |
| VS Code | `devboxes::*` (offered, members only, browser version) |
| Minecraft, FiveM | eggs with the `eula` / `cfx` features, `games::minecraft_*`, `games::fivem_enhanced` |
| Help, legal and privacy contacts, neo's extra usage | `neo::support_url`, `legal_email`, `privacy_email`, `plus_percent`, `plusplus_percent` |
| The review page, given only when asked ([reviews.md](reviews.md)) | `reviews::url`, the site's name |
| Discord invite, rewards, tier, subdomain rules | `discord::*` while the section is on; the reward amounts and the tier only without noprofit.space (it pays them then) |

- **neo on Discord** (`prompt::discord_chat`): account support, not server support. It sends server
  problems to neo in the panel. It gets the same knowledge and who it is talking to: their Discord
  name as data, whether their Discord is linked (to noprofit.space or the panel), their usage
  windows with Discord timestamps, and DM or public channel (account tools only in a DM).
- Names people chose (server, user, Discord, node, game) are always quoted, with control and
  invisible characters removed.

## Approvals

Every tool call is stored with its risk (read, write, destructive, critical), a one-line summary and,
for writes and console commands, a redacted preview (1,500 characters). Read calls run at once.
Write calls wait for the person unless they turned on making changes without asking; destructive
calls also wait when the admin keeps `neo.always_confirm_destructive` on (the default); critical
calls always wait. The person approves one call or all waiting calls, or declines one (the model is
told "The user declined this action."). Stopping a request skips everything still waiting.

Before a call runs, its permissions are checked again, and server tools (except `server_status`)
refuse while the server is suspended, installing or its install failed, restoring a backup or its
last restore failed, moving, hibernating or on a node in maintenance (`tools::console::blocked`;
`server_status` reports the same reason). A tool that fails on the server is logged; the model only hears that it failed.

## Tools

The actions neo can take are in `shared/src/neo/tools/` (`console.rs`, `files.rs`, `startup.rs`,
`backups.rs`, `servers.rs`, `mclogs.rs`, `account.rs`, `minecraft.rs`; what they share is in
`support.rs`). Each tool says where it is offered (its scope), which server permissions it needs
and how risky a call is, and goes through the same shared code as the matching page or route, so
it can do nothing the user could not do there themselves.

**Where tools are offered.** A server chat offers the *server* tools to the server's owner and to
subusers holding `neo.access`, each tool only when the user holds at least one of its permissions
(Minecraft tools only on Minecraft servers); the account chat offers the *app* tools; *any* tools
are offered in both. Approvals and the blocked states are under [Approvals](#approvals).

Every change is written to the activity log as `server:neo.<event>` or `user:neo.<event>`, with
the conversation: `command`, `power`, `file.read`, `file.edit`, `file.write`,
`file.create-directory`, `file.rename`, `file.compress`, `file.decompress`, `file.delete`,
`startup.variable`, `startup.image`, `reinstall`, `backup.create`, `backup.restore`,
`backup.delete`, `rename`, `resize`, `content.install`, `content.remove`, `content.toggle`,
`geyser`, `modpack`, `software`, and on the account `server.create` and `server.delete`. Tool
output has secrets masked (`shared/src/redact.rs`) and is cut to 12,000 characters; neo refuses to
write the `••••` mask back into a file or variable.

| Tool | Scope | Permissions | Risk | What it does |
| --- | --- | --- | --- | --- |
| `server_status` | server | none | read | State, CPU/memory/disk against the limits, uptime, address, node, and what blocks the server |
| `console_read` | server | `control.read-console` | read | The console's last lines (up to 100), colours stripped |
| `list_files` | server | `files.read` | read | A directory, folders first, with sizes and dates |
| `read_file` | server | `files.read-content` | read | A text file with line numbers, up to 400 lines per call |
| `search_files` | server | `files.read` | read | Files and folders whose name contains a pattern |
| `send_command` | server | `control.console` | write | One console command on a running server, with the console lines that followed |
| `power` | server | `control.start` / `control.stop` / `control.restart` (kill: `control.stop`) | write, kill destructive | Start, stop, restart or kill; a start goes through the [start gates](start-gates.md) and takes the user to the queue, waiting room or hibernation page |
| `edit_file` | server | `files.read-content`, `files.create` | write | Replaces an exact piece of text in a file |
| `write_file` | server | `files.create` | write | Creates a file or replaces its content (up to 256 KiB) |
| `create_directory` | server | `files.create` | write | Creates a folder |
| `rename_file` | server | `files.update` | write | Renames or moves a file or folder |
| `compress_files` | server | `files.archive` | write | Packs files of one folder into a `.tar.gz` there |
| `decompress_file` | server | `files.archive` | write | Extracts an archive into its folder |
| `delete_files` | server | `files.delete` | destructive | Deletes up to 20 files or folders, into the server's trash when it has one |
| `get_startup` | server | `startup.read` | read | Startup command, docker image and the images on offer, the visible variables (secret ones masked) with their rules |
| `set_startup_variable` | server | `startup.update` | write | Sets one editable variable, validated like the startup page |
| `set_docker_image` | server | `startup.docker-image` | write | Switches to one of the egg's images |
| `reinstall_server` | server | `settings.install` | destructive | Runs the install script again, keeping the files |
| `list_backups` | server | `backups.read` | read | The server's backups and its backup limit |
| `create_backup` | server | `backups.create` (locking: `backups.update`) | write | Starts a backup of the server's files, making room the way the backups page does |
| `restore_backup` | server | `backups.read`, `backups.restore` | critical | Restores a backup over the files, optionally deleting the current ones first |
| `delete_backup` | server | `backups.read`, `backups.delete` | destructive | Deletes a backup that is neither locked nor a system backup |
| `list_servers` | app | none | read | The user's servers (owned and shared, up to 25) with game, node and state |
| `read_mclogs` | any | none | read | A log on mclo.gs by link or id (`log`; the log check's "Ask neo" seeds it with `{"log": "<id>"}`): its problems with suggested fixes, detected versions, its error (else warning) entries |
| `account_overview` | any | none | read | Token and credit balance, the allowance with what is used, memberships, over-quota |
| `list_create_options` | any | user `servers.deploy` | read | Games (software, EULA), plans and locations for a new server, as the create page offers them |
| `create_server` | any | user `servers.deploy` | write, critical when it accepts a EULA | Creates a server like the [create page](create-server.md) |
| `rename_server` | server | `settings.rename` | write | Renames the server |
| `resize_server` | server | `settings.resize` | write | Resizes within the owner's allowance, or to a plan's size ([server-settings.md](server-settings.md)) |
| `delete_server` | server | `settings.delete` | critical | Deletes the server and its backups after the exact name is given, then takes the user home |
| `minecraft_info` | server | any of `minecraft.content`, `settings.software`, `minecraft.bedrock` | read | Software, version, installable versions, content types and sources, modpacks, switchable software, Geyser |
| `search_content` | server | `minecraft.content` | read | Searches Modrinth, CurseForge or SpigotMC for plugins, mods or modpacks |
| `content_versions` | server | `minecraft.content` | read | A project's builds that fit the server |
| `install_content` | server | `minecraft.content` | write | Installs one plugin or mod (the newest fitting build unless one is given) |
| `list_installed_content` | server | `minecraft.content` | read | The installed jars, identified, with updates and missing dependencies |
| `remove_content` | server | `minecraft.content` | destructive | Deletes up to 20 plugin or mod jars |
| `toggle_content` | server | `minecraft.content` | write | Disables (`.jar.disabled`) or enables a jar |
| `install_geyser` | server | `minecraft.bedrock` | write | Installs or updates Geyser and Floodgate and holds a Bedrock port |
| `install_modpack` | server | `minecraft.content`, `settings.software` | destructive | Installs a modpack on a stopped server in the background, switching its loader and version |
| `switch_server_software` | server | `settings.software` | destructive | Switches the Minecraft software and/or version and reinstalls |

## Limits and errors

The model call goes through `neo::turn` (providers, failover, the 5-hour and weekly windows, or
noprofit.space's gateway). What stops a turn becomes a notice in the chat, translated there from
its code: `limited_short`, `limited_long`, `limited_model`, `limited_images` (with when it resets),
`model_not_allowed` (the picked model needs a membership), `model_unavailable`, `unavailable`,
`unlinked` (noprofit.space does not know the account), `refused` (the provider's content filter).
Retry continues the same request once the cause is gone.

## Extra usage for tokens

When `limited_short` or `limited_long` ends the chat, the notice offers "One more day's worth for
2 tokens" with **Get it** (`ExtraOffer` in `elements/neo/NeoRow.tsx`): it buys, then tries the
turn again. A refused purchase says why in a toast; a 409 (too few tokens, the price or the limit
moved) adds an "Earn tokens" link to the Tokens page.

- **What a day is**: the long window's limit at the account's share (memberships raise it) divided
  by its days (`usage::day_worth`; with the defaults 15,000,000 / 7 = 2,142,857 tokens of usage).
- **What it does**: every active window gets that much room on top of what it has used now
  (`neo_usage_windows.bonus_tokens += max(0, used − (limit + bonus)) + day`, `usage::bonus_after`),
  so a turn that overshot the limit does not eat it. The bonus is added to the window's limit in
  the usage bars and the limit check (`usage::with_bonus`) and goes back to 0 when the window
  resets (`charge`).
- **When it is sold** (`neo::extra_price`): Sell Extra Usage for Tokens on, tokens on, neo metered
  by the panel (not while noprofit.space runs neo: its windows are noprofit.space's), and only
  while one of the person's windows refuses turns (else 409 "you have not reached your limit").
- **Money**: one transaction locks the user row and the windows, takes the tokens (ledger type
  `neo_extra`, "neo: one more day") and writes the bonuses; a short balance changes nothing (409).
  The chat sends the price it showed; a price that moved answers 409 with the new one. User
  activity `account:neo.extra` (`cost`, `usage`).

| Route | Permission | Does |
|---|---|---|
| `POST /api/client/account/neo/extra` | `neo.chat` and `economy.purchase` | `{tokens}` (the price seen) → `{tokens, limits}`; 403 off, 409 not sold, not limited, price moved or short |

The chat's `GET` answers `extra: {tokens, available}` (`available`: a window refuses turns now)
while it is sold to the person, else `null`.

Settings (Admin › Settings › AI › neo › Usage Limits):

| Setting | Key | Default |
|---|---|---|
| Sell Extra Usage for Tokens | `neo::extra_enabled` | on |
| Price of One More Day (Tokens) | `neo::extra_tokens` (1 to 1000) | 2 |

Check:

- With the short window at 1,000 tokens, chat until "You have reached your short-term neo limit":
  the notice shows "One more day's worth for 2 tokens" and **Get it**. With 2 tokens or more, Get
  it: the balance drops by 2, the token history shows "neo: one more day", the usage bars show the
  higher limit and the answer comes. With fewer: a toast "your balance is too low" and "Earn
  tokens".
- `POST /api/client/account/neo/extra` while no window is full: 409 "you have not reached your
  limit"; with `{"tokens": 1}` while the price is 2: 409 with the new price.
- After the window resets, its limit is the plain one again.
- Sell Extra Usage for Tokens off, or noprofit.space running neo: no offer under the notice.
- Prompt: ask neo "why does my server wait in a queue?" with the start queue on and no membership:
  the answer mentions the membership once, with `/account/membership`.

## Settings

Admin › Settings › AI › neo is one page in Pelican's order (`frontend/src/pages/admin/settings/neo/`):

- **neo on this panel**: the master switch, steps per message, the context budget, messages and
  tool runs per minute, how long chats are kept, the human support link, the legal and privacy
  contacts, the AI providers named to users and "always ask before deleting or killing".
- **AI providers and limits**: up to 6 model slots (`MAX_PROVIDERS`), added and removed freely. A
  slot has a label and description (the chat's picker), who may pick it (everyone, Plus,
  Premium), the API format (OpenAI-compatible, Anthropic Messages, Google AI Studio), the model,
  the endpoint (public https only), the key, max output tokens, a timeout, a daily token cap,
  whether it sees images, a usage multiplier, and extra headers and request fields as JSON.
  While noprofit.space runs neo (see below), the page says so and the slots are not used.
- **Usage limits**: the gateway switch, the memberships' percentages, the share for Discord
  accounts nobody linked, the short and long token windows, extra usage sold for tokens
  ([above](#extra-usage-for-tokens)), and the failover timings (cooldown
  after a failure, longest wait before an outage, time budget per turn, concurrent calls, the
  tool support check).

### Secrets in a slot

- The key never comes back to the browser. An empty field keeps the stored key, "Remove the
  stored key" deletes it, and a new slot never takes another slot's key. Keys are stored
  encrypted.
- Extra headers whose name looks like a secret (`key`, `token`, `auth`, `secret`, `cookie`,
  `session`) are shown as `••••`. Sent back as `••••`, the stored value stays; `••••` for a header
  that has no stored value is refused. The database keeps the real values.
- Header names are letters, digits and dashes (64 at most), values one line of plain text (1024
  at most), 2048 bytes in all; the key, content type and host headers are refused. An
  Anthropic slot's `anthropic-version` header (any case) replaces the default `2023-06-01`. Extra
  request fields may not set the fields the panel sets (`model`, `messages`, `tools`, `stream`,
  `max_tokens`, `contents`, `systemInstruction`…), and stay within 4096 bytes and 4 levels.
  Google AI Studio only takes `generationConfig`, `safetySettings` and `cachedContent`.

### Status and test

- `GET /api/admin/system/neo/providers` (`settings.read`): per saved slot, whether it is usable,
  until when it is skipped after a failure, its tokens today (UTC) and its last error of the
  past day. The page shows it under the slot's title while the slot has no unsaved changes.
- `POST /api/admin/system/neo/test` `{slot}` (`settings.update`): one call to the slot as saved
  (also when it is off), asking for "OK": no failover, no cooldown, no count against anyone's
  limits, at most 30 seconds. It answers `{successful, duration_ms, model, answer, tokens,
  status, error}`; the answer and error are cut to 300 characters on one line, with Google keys,
  the slot's key and its secret header values masked (the status endpoint masks the last error
  the same way). A slot that is not saved answers 404, one without an endpoint, key or model
  400. A successful test is recorded as a usage event (service `settings`, status `test`); every
  test is in the admin activity as `neo:provider-test` `{slot, model, successful, status}`.

### neo on noprofit.space

Admin › Settings › Accounts › noprofit.space has the neo gateway secret (noprofit.space's
`NEO_GATEWAY_SECRET`, at least 32 characters). While noprofit.space is connected and the secret is
set (`neo::on_noprofit`), neo runs on noprofit.space's gateway with its models and limits, and the
slots above are not used. "Remove the stored neo secret" puts neo back on this panel's models
without disconnecting noprofit.space.

## The chat

The chat looks and behaves like the Pelican panel's (`frontend/src/elements/neo/`, mounted once in
`RouterRoutes.tsx`): a pill in the window's corner ("Ask neo") opens a panel above it; on phones the
panel covers the screen and the page behind it stops scrolling. The panel can be resized from its
top-left corner (double-click resets); its size and whether it is open are remembered per browser
(`neo-chat:size`, `neo-chat:open`).

- **Which chat.** On a server's pages it is that server's chat when the person may use neo there
  (owner, or a subuser with `neo.access`; an admin is sent every permission, so only their own
  grant counts, like `may_use_on`), and nothing on the queue's waiting room; elsewhere in the
  dashboard it is the account's. There is none in the admin area, on the sign-in pages, for a
  suspended account, or while `neo.enabled` is off (`plugins/useNeo.ts`).
- **The loop** lives in `stores/neo.ts`, outside the pages: send (or approve, decline, retry), then
  stream `POST /step` while the result is `continue`, wait `retry_after` (at most 10 s) on `busy`,
  stop on anything else. A turn keeps running while the chat is minimized or the person moves
  between pages; the pill then says "Working…", and "Needs you" or "Done" (with a dot) when it
  ended unseen. Opening the chat (or loading a page with it open) resumes a turn that is still
  running. Stop cancels, and cancels once more after the step in flight. A `redirect` result (a
  start gate) closes the chat and goes to the gate's page.
- **Rows** follow Pelican's: the person's messages as bubbles, answers as markdown (the model's
  thinking folded away under "Thinking", the model's name below), each tool call as a compact tile
  with its status (a bolt when it ran without asking; a failed call's output folded away), a call
  waiting for approval as a card with its risk, its preview ("Show change"), Approve and Decline,
  and "Approve all (n)" when more than one waits. Notices are worded from their codes, with "Try
  again" on the last one while the chat is `error`, `limited` or `unlinked` (not after `refused`).
- **The foot** has the model picker (only with two models or more: each with its description, its
  usage multiplier, its own limit, the membership it needs; a locked one leads to the membership
  page when memberships are sold), the usage bars of the windows that apply (the shared windows,
  or only the picked model's own in limits by model), images waiting to be sent (picked, pasted or
  dropped; PNG, JPEG, WebP, GIF up to 20 MB, shrunk to 1568 px JPEG in the browser, at most
  `max_images`), and the message field (Enter sends on a keyboard, Shift+Enter is a new line; on
  touch screens the button sends). The send button turns into Stop while neo works. A message the
  backend did not take (blocked, too fast, a turn already running) goes back into the field.
- **Views.** History (open an earlier chat, delete one with a second tap, start a new one) and
  settings (the two switches, the usage windows, how long chats are kept and deleting all of the
  person's chats, what is sent where, the support link and the legal and privacy contacts) open
  from the head. Until the person
  agrees, the chat shows the introduction ("Before you start") instead of the conversation; with
  neo turned off, a short note and "Turn neo on".
- **"Ask neo" from a page.** `window.dispatchEvent(new CustomEvent('neo:ask', { detail: { log } }))`
  with a mclo.gs id or link (the Minecraft log check's "Ask neo to fix it") opens the server's chat
  and starts a fresh chat: what neo was doing stops, the open chat goes into the history, and the
  message about the log is sent with `seed` `read_mclogs`, so neo reads the log first. A message
  that arrives while the chat loads, before the person agreed, or while a turn is still stopping
  waits and goes out afterwards. On the account's chat the event is ignored.

## API

The chat routes are the same for the account (`/api/client/account/neo`) and a server
(`/api/client/servers/{server}/neo`), in `backend/src/routes/api/client/account/neo/chat.rs`:

| Route | Does |
|---|---|
| `GET /` | The open chat, the person's switches, why they cannot chat (if so), contacts, limits, how many days chats are kept and whether noprofit.space runs neo (`on_noprofit`: its usage windows are noprofit.space's) |
| `POST /messages` | Sends a message (`text`, `images` as data URLs, an optional `seed`: a read tool the turn starts with, used by the Minecraft log check's "Ask neo" with `read_mclogs`) |
| `POST /step` | One step, as NDJSON: `text` / `thinking` / `reset` lines while the model writes, `ping` every 15 s, then `result` with the step result and the chat |
| `POST /approve`, `/deny`, `/cancel`, `/retry` | Decide on waiting calls, stop, try again |
| `POST /new` | Starts an empty chat |
| `GET /conversations`, `GET`/`DELETE /conversations/{conversation}`, `POST /conversations/{conversation}/open` | History |

Account only (`backend/src/routes/api/client/account/neo/mod.rs`): `PATCH /preferences`,
`GET /usage` (windows and the models the person may pick), `POST /extra` (one more day's worth of
usage, [above](#extra-usage-for-tokens)), `GET /images/{image}`, and `DELETE /`
(every chat of the account; logged as `account:neo.delete-chats`). Changing the switches is logged
as `account:neo.preferences`.
