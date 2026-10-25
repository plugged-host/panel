# Admin › Users

What the fork adds to upstream's user admin.

## Deleting a user with their servers

Upstream refuses to delete a user who owns servers. The fork lets an admin take the servers along.

- `DELETE /api/admin/users/{user}?delete_servers=true` deletes every server the user owns (native
  server delete, files included; backups stay), then the user. It needs `users.delete` and
  `servers.delete`. `&force=true` deletes servers from the panel even when their node cannot be
  reached, like the server delete's own "force".
- If a server cannot be deleted (without `force`), the request stops there with that server's name;
  the servers deleted before it stay deleted and the user stays.
- Without `delete_servers` the route behaves as upstream's.
- The `user:delete` activity entry lists the deleted servers (`deleted_servers`).

In Admin › Users › a user › Delete, the confirmation has "Delete their servers too" (shown with
`servers.delete`) and, when it is on, "force".

## Discord column and search

Admin › Users (and a role's Users tab) has a **Discord** column, as the Pelican panel's user list
had: the linked Discord account's name, its id in the tooltip, a click copies the id. The search
also finds users by their Discord id (exact) or name (part of it).

- With noprofit.space connected, the Discord account comes from the account snapshot
  (`user_noprofit_accounts.discord`); standalone, from the Discord OAuth link (Admin › Settings ›
  Discord's provider, `user_oauth_links`), named from `discord_members` (the member check).
- `shared/src/user_links.rs` loads it for a whole page with one query (`load`, `attach`) into the
  admin user's `links` (`{noprofit, discord: {id, username}}`); only the users list, the user page
  and a role's users fill it, everywhere else it is `null`. The search is
  `user_links::discord_search_sql`, OR-ed into `User::all_with_pagination`.

## Identity lock (noprofit.space)

While noprofit.space is connected, sign-in details are noprofit.space's, as on the Pelican panel:

- **No panel-made users.** `POST /api/admin/users` answers 409 "users are created by signing in
  with noprofit.space", and Admin › Users has no Create button. Emergency admins (panel password,
  `/auth/login?local=1`) are made with the `users create` command.
- **Linked users keep noprofit.space's username and email.** `PATCH /api/admin/users/{user}` that
  changes either answers 409 "username and email are managed on noprofit.space"
  (`user_links::identity_locked`); the user page shows both fields disabled with "Managed on
  noprofit.space, where this user signs in." Everything else (name, language, role, admin, frozen,
  suspended, password for the emergency sign-in) stays editable. The account sync keeps writing
  them.
- Users without a noprofit.space link (emergency admins) are edited as upstream does.
- **Deleting goes the other way.** Deleting a linked user here leaves the noprofit.space account;
  deleting the account on noprofit.space deletes the user here with its servers and their backups
  ([noprofit.md](../noprofit.md#what-changes-while-it-is-connected)).

Without noprofit.space nothing is locked and Create works as upstream's.

## Check

- Delete a test user who owns a server, with "Delete their servers too": the user and the server are
  gone, the activity log lists the server.
- Without the switch: the panel answers "user has servers, cannot delete" as before.
- Admin › Users shows a Discord column; hovering a name shows the id, clicking copies it; searching
  a Discord id or part of a Discord name finds the user.
- With noprofit.space connected: no Create button; a linked user's page has username and email
  disabled with the note; saving other fields works. An emergency admin's page is fully editable.
- Without noprofit.space: Create is back and every field is editable.
