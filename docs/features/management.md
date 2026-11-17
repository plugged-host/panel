# Management (staff roles)

A panel run by a team gives most of the team a **role** instead of the admin flag: they get the
admin permissions they need and nothing else. A role never needs the panel's settings to do staff
work. Without `settings.update` (and `email-templates.update`, `extensions.manage`) nobody with that
role can change the economy, neo, the ads, mail or any other Admin › Settings tab. The only route
that writes the settings is `PUT /api/admin/settings`, and it checks `settings.update`.

The admin flag stays for the owners. With noprofit.space connected it comes from there
([noprofit.md](../noprofit.md)), and roles are given on Admin › Users (by an admin, or by staff
within the rules below).

## What staff can be given

Every admin permission upstream has, plus plugged.host's (`status`, `abuse`, `tickets`,
`suggestions`, `partners`, `plans`, `domains`, `ad-banners`, `users.economy`, …). On top of those,
the rules that bind players on a server's own pages have a permission of their own in the `servers`
group. Before this, only the admin flag could lift them:

| Permission | Lets the holder, on any server's own pages |
|---|---|
| `servers.resize` | Resize it past its owner's allowance, also while players may not resize (`games.allow_resizing`) |
| `servers.remove` | Delete it while players may not delete theirs (`games.allow_deleting`) |
| `servers.move` | Move it while moves are off, during the cooldown or while it is suspended |
| `servers.limbo` | Restore it from limbo without tokens and to any location |
| `servers.start-gates` | Add schedule steps that start it, import such schedules, start it right after a reinstall ([start-gates.md](start-gates.md)) |
| `servers.startup` | Set any custom startup command: only the syntax is checked, not the abuse rules ([startup-commands.md](startup-commands.md)) |

The handlers ask `PermissionManager::can_override(permission)` (`shared/src/models/user/auth.rs`):
an administrator, or a role or API key that holds the permission. A role also needs the server
permission for the action (`settings.resize`, `settings.delete`, …), like any admin acting on
someone else's server.

## Safe by construction

- **No escalation through roles.** A role can only be created, changed or duplicated with
  permissions its author holds (`can_grant`, upstream's).
- **No take-over of stronger accounts.** Changing a user (profile, email actions, 2FA, OAuth links,
  economy) and impersonating one need the caller to hold that user's whole role
  (`can_modify_user`, `can_impersonate`). Administrators are only changed by an administrator
  signed in with a session.
- **Handing out roles.** Staff with `users.update` may give or take a role only if they hold the
  whole role they give, and never to themselves. The admin flag stays with administrators.
- **No paying yourself.** `users.economy` never changes the caller's own credits, tokens or
  memberships unless the caller is an administrator.
- A role can require two-factor authentication (upstream's switch on the role).
- Every staff action is in the admin activity log under the staff member's name.

## Admin › Roles

- **Staff Preset** fills in every admin permission except the settings ones (`settings.*`,
  `email-templates.*`, `extensions.*`, `oauth-providers.*`), changing roles (`roles.create`,
  `roles.update`, `roles.delete`; `roles.read` stays, to hand roles out), node tokens and
  upgrades (`nodes.read-token`, `nodes.reset-token`, `nodes.upgrade`) and `servers.startup`, plus
  every server permission. Untick what the role should not have.
- A role that can change settings shows a warning above the permissions.

## Check

- Make a role with Staff Preset and give it to a test account. It sees every admin page except the
  Settings pages, and `PUT /api/admin/settings` answers 403.
- As that account: resize a player's server past its allowance (works), change a full admin's email
  (403), give another user the same role (works), give yourself another role (403), change your own
  tokens (403).
- Remove `servers.resize` from the role: the player-page resize is back to the allowance.
