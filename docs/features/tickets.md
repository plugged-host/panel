# Support tickets

Players get help from the panel instead of Discord: Account › Support lists their tickets, opens a
full thread and keeps the server's context when a person takes over. neo answers the first message
from the server's facts when it is on; the team answers as staff on Admin › Tickets. In
`shared/src/tickets.rs`.

## A ticket

A ticket has a subject, a category (`general`, `billing`, `technical`, `abuse`, `other`), an optional
server, a status (`open`, `waiting`, `resolved`, `closed`) and a priority (`low`, `normal`, `high`).
The thread keeps every message with its author: the person (`user`), staff (`admin`), neo (`neo`) or
the panel (`system`). Closing is the person's or staff's decision; a reply to a resolved ticket opens
it again, a closed one must be reopened first.

The server is one the account owns or is a subuser of; the panel stores its UUID with the ticket, so
staff still see which server it is about after the person deletes it (`server_uuid` becomes `NULL`).

## neo's first answer

When a ticket is created, `tickets::spawn_neo_reply` runs in the background:

1. It takes a cache lock for the ticket and stops when a `neo` or `system` message exists already.
2. It builds facts from the server only (name, game, node, location, address, limits, suspended or
   hibernated, install state). It never reads files or the console.
3. It asks neo with no tools (`neo::chain::Chain`, scope `tickets`, 60 seconds) and stores the answer
   as a message of type `neo`. The ticket body and subject are fenced as untrusted data.
4. When neo is off, unlinked or unavailable, it writes a short `system` acknowledgement instead.

The UI labels `neo` messages "Assistant" and `system` messages "Panel".

## Account › Support

The page lists the account's tickets (subject, status, category, server, last update) newest update
first. **New ticket** is the only filled action: subject, category, optional server, first message.
A row opens the thread: every message with its author and time, a reply box, Close, and Reopen when
it is closed.

## Admin › Tickets

`tickets.read` shows every ticket; `tickets.manage` acts. The list is paginated and filters by status
and category, with a search over the subject, the account and the server. A row opens the thread:
staff reply, assign the ticket to themselves (or clear it), change status and priority.

## Notifications

- New ticket: admins with `tickets.manage` get `admin.ticketCreated` (`subject`, `category`, `user`).
- Staff reply: the account gets `ticketReply` (`subject`).

## Permissions

| Permission | For | Allows |
|---|---|---|
| `tickets.read` | user | The account's tickets and their messages |
| `tickets.create` | user | Opening a ticket, replying, closing and reopening |
| `tickets.read` | admin | Every ticket and its server context |
| `tickets.manage` | admin | Replying as staff, assigning, status and priority |

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/account/tickets` | `tickets.read` | `{tickets}`: the account's tickets |
| `POST /api/client/account/tickets` | `tickets.create` | `{subject, category, server_uuid?, body}` |
| `GET /api/client/account/tickets/{ticket}` | `tickets.read` | `{ticket, messages}` |
| `POST /api/client/account/tickets/{ticket}/reply` | `tickets.create` | `{body}` |
| `POST /api/client/account/tickets/{ticket}/status` | `tickets.create` | `{status}`: `open` or `closed` |
| `GET /api/admin/tickets` | `tickets.read` | Paginated; `status`, `category`, `search`, `assigned` (`me`) |
| `GET /api/admin/tickets/{ticket}` | `tickets.read` | `{ticket, messages}` |
| `POST /api/admin/tickets/{ticket}/reply` | `tickets.manage` | `{body}` as staff |
| `POST /api/admin/tickets/{ticket}/assign` | `tickets.manage` | Assigns the caller |
| `DELETE /api/admin/tickets/{ticket}/assign` | `tickets.manage` | Clears the assignment |
| `PATCH /api/admin/tickets/{ticket}` | `tickets.manage` | `{status?, priority?}` |

Activity: `account:ticket.create`, `.reply`, `.status`; `tickets:ticket.reply`, `.assign`,
`.unassign`, `.update`.

## Tables

- `tickets`: `uuid`, `user_uuid`, `server_uuid` (`NULL` when the server is gone), `subject`,
  `category`, `status`, `priority`, `assigned_to`, `created`, `updated`, `closed`.
- `ticket_messages`: `uuid`, `ticket_uuid`, `author_uuid` (`NULL` for neo and the panel),
  `author_type` (`user`, `admin`, `neo`, `system`), `body`, `created`.

Migration `tickets_suggestions_custom_domains`.

## Without noprofit.space

Nothing changes. neo is optional; without it every ticket gets the panel's acknowledgement and a
person answers.

## Checks

- Account › Support: New ticket with and without a server; it appears at the top. With neo on, its
  answer is labelled Assistant; with neo off, the panel's acknowledgement appears.
- Open the ticket: the whole thread, reply, Close, Reopen. A reply to a closed ticket is refused.
  There is no delete button on the account pages; deletion is admin-only from Admin › Tickets.
- Admin › Tickets: the new ticket is first among open ones; filter by category and status, search the
  subject and the account. Reply as staff: the person gets `ticketReply`; assign yourself and clear
  it; set Waiting, Resolved and Closed.
- A server ticket keeps its server name in both lists; delete the server: the ticket stays, without
  the server.

## Deeper tickets (v3.1)

- A ticket opens as a full page (`/account/support/{id}`, `/admin/tickets/{id}`), not a modal.
- Category `bug` adds steps to reproduce, expected result, actual result and environment.
- Owners can attach up to six images/PDF/text/JSON/zip files (10 MB each) when opening a ticket;
  they are stored through the panel's storage driver and shown inline in the thread.
- neo reads the whole thread (not just the first message) and the server facts. It has the normal
  account/server tools, may run read and write tools (not destructive/critical ones), and has
  `unsuspend_server` for a server the user owns. An abuse suspension is checked first: the server
  must pass a fresh abuse scan (critical findings or an unreachable node mean no). An admin's
  suspension is only lifted when a staff message in the ticket explicitly allows or asks for it;
  the owner's own request is not enough. A noprofit.space suspension is never lifted by neo.
- Admins can delete a ticket from Admin › Tickets (thread and attachments go with it). Owners
  cannot delete their tickets or ideas from the account pages.

## Live thread, notifications and escalation

- The open ticket polls every 2 seconds; lists every 10. neo's answer is written into a
  placeholder message while it is generated (deltas every ~400 ms), so it grows instead of
  appearing only after a refresh. An empty neo message shows "neo is writing…".
- Every neo reply and staff reply writes a notification; the bell polls every 10 seconds.
- neo can call `escalate_ticket`: it marks the ticket waiting/high priority, writes a system line,
  notifies the admins with `tickets.manage`, notifies the opener, and rux DMs the opener when a
  Discord account is linked.
