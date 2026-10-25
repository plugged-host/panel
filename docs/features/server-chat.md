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

Server › Chat, below Activity. It opens on the newest page (50 messages) and reads top to bottom;
**Load older messages** fetches the page before it. The page polls every 5 seconds while it is open
and scrolls to the bottom when new messages arrive and the reader is already there. A message is at
most 2000 characters; empty messages are refused. A message whose author has since been deleted
stays, without a name or avatar.

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
  answer. Remove `chat.write` from the subuser: the input disappears and the POST is refused.
- Write more than 50 messages: the oldest page is reachable with **Load older messages**.
- Delete the author of a message in Admin › Users: the message stays, without a name.
- Delete the server: its chat rows are gone (`server_chat_messages_server_uuid_fkey`, cascade).
