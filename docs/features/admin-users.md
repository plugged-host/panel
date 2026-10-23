# Deleting a user with their servers

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

## Check

- Delete a test user who owns a server, with "Delete their servers too": the user and the server are
  gone, the activity log lists the server.
- Without the switch: the panel answers "user has servers, cannot delete" as before.
