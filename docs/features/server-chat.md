# Server chat

A chat between a server's owner and its subusers, on the server's own sidebar
(`shared/src/models/server_chat.rs`, `backend/src/routes/api/client/servers/_server_/chat.rs`,
`frontend/src/pages/server/chat/ServerChat.tsx`). It is for the people who share a server, not for
players in game.

## Permissions

Two server permissions, in the **chat** group:

| Permission | Allows |
|---|---|
| `chat.read` | Reading the server's chat |
| `chat.write` | Writing in it (reading is not implied) |

The owner has every permission already. The migration `server_chat_permissions` adds both to every
existing subuser, so nobody loses access when the page appears. A subuser's permissions are edited
on the server's Subusers page as before; the permission picker shows the new group.

## The page

Server › Chat, below Activity, drawn like neo's chat (`.theme-chat-*` in `app.css`, the composer
is neo's `.theme-neo-compose`): one card that reaches down to the window's foot (measured, so it
ends above the keyboard on a phone and above neo's launcher in the corner), the thread in it and
the composer at its foot.

- **Messages.** One's own sit at the line's end as neo's user bubbles; everyone else's sit at the
  start as text beside their avatar. One person's messages at most 5 minutes apart form a run
  under one name and time (`groupChat` in `frontend/src/lib/serverChat.ts`); a deleted account's
  messages have no name, a blank avatar and never join a run. Message text keeps its line breaks
  and takes its direction from its own script (`dir='auto'`).
- **Days and times.** Each day starts with a hairline and its label: Today, Yesterday, the weekday
  within the last week, else the date (with the year when it is another one), in the panel's
  language. A time reads "now", "5 minutes ago" within the hour, then the time of day; the full date
  and time are in its tooltip.
- **Scrolling.** It opens at the newest message. New messages scroll into view only while the
  reader is at the bottom; further up, the view stays put and a **New messages** pill at the bottom
  jumps down. Scrolling to the top loads the page before (50 messages) with the reader's place
  kept; **Load older messages** at the top does the same by hand. The page polls every 5 seconds
  while it is open; a failed poll keeps the messages on screen.
- **Writing.** The field grows with its text; Enter sends and Shift+Enter starts a new line (on a
  touch screen Enter is a new line and the round send button sends, as in neo). A message is at most
  2000 characters; a counter shows from 1800 on. A sent message shows at once, faded until the panel
  has stored it; if sending fails it goes back into the field. Without `chat.write` the foot says
  that the person can read the chat but not write in it.
- **Empty, loading, failed.** An empty chat says so in one muted line; loading shows the spinner;
  a chat that cannot be loaded shows the error with **Retry**.

The chat is not the activity log: writing a message does not create an activity entry, and the
activity log is not shown in the chat.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/chat?page=&per_page=` | `chat.read` | `{messages}`: a page of `{uuid, user, message, created}`, newest page first, each page oldest to newest |
| `POST /api/client/servers/{server}/chat` | `chat.write` | `{message}` → `{message}` (the stored message with its author); 400 when empty |

`user` is the author as the panel shows people elsewhere (`uuid`, `username`, `avatar`, …), or
`null` for a deleted account.

## Tables

| Table / columns | What it holds |
|---|---|
| `server_chat_messages` | `uuid`, `server_uuid` (deleted with the server), `user_uuid` (set to null when the account is deleted), `message`, `created` |

Indexes: `(server_uuid, created)` for a page, `user_uuid` for the account's messages.

## Check

- As the owner, write in the chat; as a subuser with `chat.read` and `chat.write`, see it and
  answer: your own messages are bubbles at the end, the other person's are text with their avatar,
  name and time at the start. Remove `chat.write` from the subuser: the foot says they can read but
  not write, and the POST is refused.
- Send three messages within a minute: one time above them, the bubbles stacked. Wait more than 5
  minutes and send another: a new run with its own time.
- Enter sends, Shift+Enter makes a new line; past 1800 characters the counter appears, and the
  field stops at 2000.
- Scroll up a little and have the other person write: the view does not move and **New messages**
  appears; clicking it jumps to the bottom. At the bottom, their message scrolls into view.
- Write more than 50 messages, reload and scroll to the top: the older page loads above without the
  view jumping; **Load older messages** does the same.
- On a phone: the chat fills the screen under the page header, the keyboard pushes the composer up
  with it, and neo's button does not cover the send button.
- In Arabic: the layout mirrors (your bubbles on the left), the day labels and times are Arabic, and
  a message in English still reads left to right.
- Delete the author of a message in Admin › Users: the message stays, without a name.
- Delete the server: its chat rows are gone (`server_chat_messages_server_uuid_fkey`, cascade).
