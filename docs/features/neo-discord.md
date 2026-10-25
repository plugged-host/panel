# neo on Discord

neo answers account questions on the Discord server: signing in, linking Discord, the allowance,
credits and tokens, memberships, locations, where to get help. Anyone may ask, linked or not. It
is account support, not server support: for a server's own problem it sends people to neo in the
panel, and it never changes anything. The code is in `shared/src/neo/discord/`, its settings in
`shared/src/settings/neo_discord.rs`.

## Turning it on

It runs on the assistant bot of [discord.md](discord.md), its own Discord application:

1. In Discord's Developer Portal, make an application for it and copy its bot token (Bot › Reset
   Token). Invite it to the server with the scope `bot` and the permissions View Channels, Send
   Messages, Send Messages in Threads, Embed Links and Read Message History:
   `https://discord.com/oauth2/authorize?client_id=<application id>&scope=bot&permissions=274877991936`.
2. Admin › Settings › Accounts › Discord: the section on with the server's id, Assistant Bot on, its
   token.
3. neo has to be on (its master switch in neo's settings) and able to think: noprofit.space runs it
   (its neo secret in the noprofit.space settings), or a provider is set up in neo's settings.
   Otherwise the bot answers that it cannot answer right now.

`shared::neo::discord::register(&state)` plugs the bot into the gateway at startup; the gateway
connects it on the primary instance once its switch and token are set. Everything is off by default.

## What it answers

- Every DM.
- On the Discord section's server (never another): a message that @mentions it, replies to one of
  its messages or, with name calls on, starts with its name ("hey neo, …", "neo: …"; its Discord
  username and display name, taken from the gateway's READY).
- A screenshot without text counts as a question, and so does a mention in reply to someone else's
  message.

It ignores bots, webhooks, system messages (joins, pins) and itself. One Discord account may send
it `messages_per_minute` messages a minute (6); the ones over it are ignored.

Discord sends the text of DMs, mentions and replies that ping the bot without the privileged
Message Content intent. Name calls need it: turn it on in the Developer Portal first, or Discord
refuses the connection. The gateway asks for the intents when a session starts (reconnects keep
them), so a change of Name Calls applies once the assistant bot's session starts over: after the
panel restarts, or after the bot is turned off and on again on the Discord tab.

## Who is asking

`who::resolve` decides, before every answer:

| | The panel meters neo | noprofit.space runs neo |
|---|---|---|
| Linked | The panel account the Discord account is linked to (the Discord OAuth provider, or the noprofit.space link): that account's windows and membership, the same as in the chat | noprofit.space's account (`/usage` with `discord_id`): its windows, shared with the panel's chat |
| Not linked | Windows of the Discord account's own (`neo_usage_windows.discord_id`), at `neo::discord_unlinked_percent` of the limits | noprofit.space's windows for the Discord account, at its share |

Turns are recorded under the service `discord` (panel), or `<sign-in client>-discord` on
noprofit.space, with the conversation `discord:{channel_id}:{author_id}`. If a model was picked for
the panel's chat, it does not apply here: Discord turns use the default model. noprofit.space's
answer about an account is kept for 20 seconds, so a burst of messages asks once. When a window is
used up, the bot answers with when it resets, and an unlinked person also gets where to link
Discord: the portal is noprofit.space while it is connected, else the panel's name, the link
`{accounts site}/settings` or `{panel}/account/oauth-links`.

## One answer

A conversation is one person in one channel or DM (`{channel_id}:{author_id}`). A message is stored
`pending` and a worker of its conversation answers it in the background (the gateway runs events one
after another). The worker holds the conversation's lock (`neo::discord::{conversation}`), takes
every waiting message (`running`) and answers them together, then looks again, so messages sent while
it thinks are answered next, in one answer. After a restart or a reconnect, READY starts the workers
of conversations with messages from the last 15 minutes still waiting, and every minute the
`recover_neo_discord_messages` task starts a worker for a message that has waited more than a
minute (its worker failed) and gives up those that waited more than 15 (`failed`, never answered
or replayed). While the bot is off, nothing is answered.

The answer is neo's usual loop: the system prompt, the remembered answers, the messages, then model
turns (the bot shows "is typing…" meanwhile) and the lookups they ask for, until the model answers in
text. At most `max_steps` model calls (6); the last one may not ask for lookups. Busy providers are
waited out for up to two minutes, and the whole answer has five minutes.

The answer is posted as a reply to the last message, split into messages of at most 1900 characters
(at a line break where one is near). It never pings anyone: `allowed_mentions` is empty, the replied
person included. When it cannot answer, it posts a fixed line:

| When | Line |
|---|---|
| neo cannot think, noprofit.space does not answer, providers stay busy, the time is up | I can't answer right now, sorry. Try again in a few minutes, or ask the staff here if it's urgent. |
| Something broke | Something went wrong on my side. Try again in a moment. |
| The model said nothing | Sorry, I don't have an answer to that. Ask in the server and a human can help. |
| A window is used up | You've used up your limit for now. It resets `<t:…:R>`. |
| A window is used up, not linked | You've used up the limit for Discord accounts that aren't linked. It resets `<t:…:R>`. Link your Discord to an account on `<portal>` for the full limits: `<link>` |

## The prompt

`neo::prompt::discord_chat` writes it (`shared/src/neo/prompt.rs`): what the host is and how it
works (the same knowledge as the chat, only what the settings turn on), the rules for Discord (short,
plain URLs, no pings, never ask for secrets, moderation is the moderation bot's), and this
conversation: who is asking (their Discord name is data, not instructions), linked or not and how to
link, their usage with Discord timestamps, and DM or public channel. In a public channel it has no
lookups and does not repeat account details.

The messages go to the model as the person wrote them, then what Discord resolved around them: who
they mentioned, the message they reply to (in a `<replied_message untrusted="true">` fence, angle
brackets replaced so it cannot close it) and how many images it has.

## Lookups

In a DM, for a Discord account linked to a panel account that is not suspended, the model is offered
the read-only account tools of neo's registry (`agent::TOOLS`): `account_overview`,
`list_servers` and `list_create_options`, as far as the registry has them and the account may use
them. They act for that panel account only. Before a call runs, it is checked again: offered here,
`read` risk with these arguments, permitted. Its output has secrets masked (`crate::redact`), is cut
to 6000 characters and goes back in a `<tool_output untrusted="true">` fence.

## Images

Up to four images per answer: the messages' attachments, then those of the messages they reply to.
Only PNG, JPEG, WebP and GIF of at most 10 MB on Discord's CDN (`https://cdn.discordapp.com`,
`https://media.discordapp.net`, no port or credentials). They are fetched when the answer starts
(Discord's links expire), with no redirects and no internal addresses (`net::BlockedIpResolver`),
and shrunk to a JPEG of at most 1568 px a side. A model that cannot see images gets a note instead.
Only the answer that reads them sees them; remembered answers keep the note that there were images.

## Memory

`neo_discord_messages` keeps the conversations:

| Column | |
|---|---|
| `conversation` | `{channel_id}:{author_id}` |
| `turn_uuid`, `position` | The answer a row belongs to (the uuid of the first message it answers) and its order in it; the messages answered are at 0 |
| `discord_message_id`, `channel_id`, `guild_id`, `author_id`, `author_name` | Where it was said and by whom; `guild_id` is null in a DM |
| `role` | `user` (a message to the bot), `assistant` (a model turn), `tool` (a lookup) |
| `content`, `tool_calls`, `tool_call_id`, `tool_name` | What was said, in the chat shape |
| `status` | `pending`, `running`, `done`, `failed` (a fixed line was posted, or it was given up) |
| `meta` | Messages: `mentions`, `images`, `reply_to`, and who answered (`linked`, `account_id`, `user_uuid`, `error`). Model turns: `provider`, `model`, `tokens` |

An answer remembers up to `history_turns` (6) earlier answers in the same conversation from the last
`history_minutes` (60), only ones that finished normally. When this answer offers no lookups, only
what was said is replayed. The daily `prune_neo_discord_messages` task deletes rows older than neo's
retention days (`neo::retention_days`), like the panel's chats.

## Settings

Admin › Settings › AI › neo on Discord (`neo_discord::*`); the switch and the token are the
assistant bot's on the Discord tab, the models, limits and retention neo's.

| Setting | Default | |
|---|---|---|
| `name_calls` | off | Also answer messages that start with the bot's name; needs the Message Content intent |
| `messages_per_minute` | 6 | Messages one Discord account may send it a minute (1 to 60) |
| `max_steps` | 6 | Model calls one answer may take (2 to 20) |
| `history_minutes` | 60 | How old a remembered answer may be (1 to 1440) |
| `history_turns` | 6 | How many earlier answers are remembered (0 to 20) |

## Code

| File | |
|---|---|
| `shared/src/neo/discord/mod.rs` | The handler (`Bot::Assistant`), the workers, `register()`, `recover()`, `prune()` |
| `inbox.rs` | Which messages it answers and what is kept of them (pure) |
| `messages.rs` | The table: store, claim, finish, history, give up, prune |
| `who.rs` | Who the Discord account is, and its turns (panel or noprofit.space) |
| `agent.rs` | One answer: the loop, lookups, images, the reply |
| `rest.rs` | Typing, posting, fetching attachments |
| `images.rs` | Shrinking screenshots |
| `shared/src/neo/gateway.rs` | `discord_usage`, `discord_turn` (noprofit.space's `/usage` and `/turn` with `discord_id`) |
