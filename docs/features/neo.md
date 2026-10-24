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
  settings (the two switches, the usage windows, deleting all of the person's chats, what is sent
  where, the support link and the legal and privacy contacts) open from the head. Until the person
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
