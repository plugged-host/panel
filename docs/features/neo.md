# neo

neo is the assistant in the panel's chat: a model that reads a server's console and files and acts
on them with the person's own permissions, asking before it changes anything unless the person
allowed it to go ahead. The chat runs on the account (account questions, the store, servers in
general) and on each server (that server). Its code is in `shared/src/neo/`; the models it talks
to (provider slots, the noprofit.space gateway, the usage windows) are described under
[Models and limits](#models-and-limits) below.

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

## Approvals

Every tool call is stored with its risk (read, write, destructive, critical), a one-line summary and,
for writes and console commands, a redacted preview (1,500 characters). Read calls run at once.
Write calls wait for the person unless they turned on making changes without asking; destructive
calls also wait when the admin keeps `neo.always_confirm_destructive` on (the default); critical
calls always wait. The person approves one call or all waiting calls, or declines one (the model is
told "The user declined this action."). Stopping a request skips everything still waiting.

Before a call runs, its permissions are checked again, and server tools (except `server_status`)
refuse while the server is suspended, installing, restoring a backup, moving, hibernating or on a
node in maintenance. A tool that fails on the server is logged; the model only hears that it failed.

## Limits and errors

The model call goes through `neo::turn` (providers, failover, the 5-hour and weekly windows, or
noprofit.space's gateway). What stops a turn becomes a notice in the chat, translated there from
its code: `limited_short`, `limited_long`, `limited_model`, `limited_images` (with when it resets),
`model_not_allowed` (the picked model needs a membership), `model_unavailable`, `unavailable`,
`unlinked` (noprofit.space does not know the account), `refused` (the provider's content filter).
Retry continues the same request once the cause is gone.

## API

The chat routes are the same for the account (`/api/client/account/neo`) and a server
(`/api/client/servers/{server}/neo`), in `backend/src/routes/api/client/account/neo/chat.rs`:

| Route | Does |
|---|---|
| `GET /` | The open chat, the person's switches, why they cannot chat (if so), contacts and limits |
| `POST /messages` | Sends a message (`text`, `images` as data URLs, an optional `seed`: a read tool the turn starts with, used by the Minecraft log check's "Ask neo" with `read_mclogs`) |
| `POST /step` | One step, as NDJSON: `text` / `thinking` / `reset` lines while the model writes, `ping` every 15 s, then `result` with the step result and the chat |
| `POST /approve`, `/deny`, `/cancel`, `/retry` | Decide on waiting calls, stop, try again |
| `POST /new` | Starts an empty chat |
| `GET /conversations`, `GET`/`DELETE /conversations/{conversation}`, `POST /conversations/{conversation}/open` | History |

Account only (`backend/src/routes/api/client/account/neo/mod.rs`): `PATCH /preferences`,
`GET /usage` (windows and the models the person may pick), `GET /images/{image}`, and `DELETE /`
(every chat of the account; logged as `account:neo.delete-chats`). Changing the switches is logged
as `account:neo.preferences`.
