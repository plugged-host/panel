# Suggestions

A board of ideas for the panel: every account may post an idea and vote for as many as it likes,
one vote per idea. Ideas are sorted by votes and then newest. Administrators move them through the
statuses, write the changelog entry when one ships and merge duplicates. In
`shared/src/suggestions.rs`.

## The board

Account › Suggestions shows every idea that is not merged: title, one-sentence description, server
type (`minecraft`, `fivem`, `rust`, `teamspeak`, `other`), author and date, a vote button with the
count, and the status:

| Status | Shown as |
|---|---|
| `under_review` | Under review |
| `planned` | Planned |
| `in_progress` | In progress |
| `shipped` | Shipped |
| `declined` | Declined |
| `merged` | Merged (hidden from the board) |

A shipped idea shows its changelog entry ("Shipped: …"). **New idea** is the only filled action:
title (3 to 120 characters), description (10 to 500) and server type. The vote button toggles the
account's vote; voting again takes it back.

## Admin › Suggestions

`suggestions.read` shows every idea; `suggestions.manage` acts. The list is paginated and filters by
status and server type, with a search over the title, the description and the author.

- **Edit**: status and changelog entry. `merged` is not picked by hand; it comes from a merge.
- **Merge**: the votes move to the chosen idea (`ON CONFLICT DO NOTHING` for accounts that voted for
  both), the source is marked `merged` with `merged_into`, and it disappears from the board.
- **Delete**: the idea and its votes.

New ideas notify admins with `suggestions.manage` (`admin.suggestionCreated`).

## Anti-alt

Before a vote is counted, the panel asks noprofit.space's signed `/alt/check` when the account is
linked (docs/noprofit.md#ultra-and-anti-alt). `limited: true` means this person's vote already counted, so
the vote is refused with "your vote already counted for this person". A failed check never blocks the
vote. A counted vote sends `/alt/signal` (`kind: suggestion_vote`) best effort.

## Permissions

| Permission | For | Allows |
|---|---|---|
| `suggestions.read` | user | The board |
| `suggestions.create` | user | Posting an idea |
| `suggestions.vote` | user | Voting and taking the vote back |
| `suggestions.read` | admin | Every idea and its votes |
| `suggestions.manage` | admin | Status, changelog, merge and delete |

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/account/suggestions` | `suggestions.read` | `{suggestions}`: the board |
| `POST /api/client/account/suggestions` | `suggestions.create` | `{title, body, server_type}` |
| `POST /api/client/account/suggestions/{suggestion}/vote` | `suggestions.vote` | Toggles the account's vote |
| `GET /api/admin/suggestions` | `suggestions.read` | Paginated; `status`, `server_type`, `search` |
| `PATCH /api/admin/suggestions/{suggestion}` | `suggestions.manage` | `{status, changelog?}` |
| `POST /api/admin/suggestions/{suggestion}/merge` | `suggestions.manage` | `{target_uuid}` |
| `DELETE /api/admin/suggestions/{suggestion}` | `suggestions.manage` | With its votes |

Activity: `account:suggestion.create`; `suggestions:suggestion.update`, `.merge`, `.delete`.

## Tables

- `suggestions`: `uuid`, `user_uuid`, `title`, `body`, `server_type`, `status`, `changelog`,
  `merged_into`, `created`, `updated`.
- `suggestion_votes`: `suggestion_uuid`, `user_uuid` (primary key, one vote per account and idea),
  `created`.

Migration `tickets_suggestions_custom_domains`.

## Without noprofit.space

Nothing changes. The anti-alt check is skipped, so every account votes once as before.

## Checks

- Account › Suggestions: post an idea; it appears. Vote: the count goes up and the button stays
  filled; vote again: down. A second account votes too; the order follows the counts.
- Admin › Suggestions: set Planned, then Shipped with a changelog entry; the board shows the status
  and the entry. Merge one idea into another: its votes move, it disappears, and the target's count
  is the sum (minus accounts that voted for both).
- Delete an idea: it and its votes are gone.
- With noprofit.space connected and an alt-limited account, voting is refused with the message; with
  the service unreachable, voting still works.
