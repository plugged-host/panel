# Welcome tour

A short guide drawn over the real panel: the page is dimmed, the part a step is about stays lit, and
the step's text sits next to it. Someone told "open Settings" afterwards knows where Settings is.
Off by default; Admin › Settings › Accounts › Welcome Tour turns it on.

## The two parts

The tour runs on the pages it explains, so it has two parts:

| Part | Where | Steps |
|---|---|---|
| Panel | the server list (`/`, `/all`, `/grouped`) | welcome, the sidebar, the account menu, then Create Server and the first server card, or (no server yet) the empty list and Create Server |
| Server | the next console the user opens after the first part | console, power buttons, address and usage, the server's sidebar, Settings (when the user may open it), the logo back home |

The first part ends with "Show me {server}" (opens the first server, where the second part starts),
"Create my first server" (opens `/create`) or "Finish". "Skip tour" ends both parts; the second
part's "Not now" ends that part. A step whose element is not on screen (a phone hides the sidebar)
shows its text in the middle of the window, and on phones the card sits at the bottom.

"Replay welcome tour" in the menu under the user's name starts both parts again. Escape closes the
current part like its "Skip tour" / "Not now" button.

## State

Per user in `user_welcome_tours` (`shared/src/welcome_tour.rs`): the version it was written for,
the step of each part, and when the first part was completed or skipped and the second one closed.
A row for an older version counts as none: raising **Tour Version** in the settings shows the tour
again to everyone, for example after the panel changed.

| Route | |
|---|---|
| `GET /api/client/account/tour` | `{tour: {panel_active, server_active, step, server_step}}` |
| `POST /api/client/account/tour` | `{action, step}`; action is `advance`, `advance_server` (with `step`), `complete`, `skip`, `complete_server` or `restart` |

With the tour turned off, both parts are inactive and the menu entry is hidden; the stored steps
stay, so turning it on again continues where each user was.

## Settings

Admin › Settings › Accounts › Welcome Tour (`tour::enabled`, `tour::version`).

## Without noprofit.space

Nothing differs; the tour does not mention noprofit.space.

## Check

- Admin › Settings › Accounts › Welcome Tour: turn it on and save. On the server list, the tour
  starts with "Welcome to {app}"; Next lights the sidebar, then the account row at its foot, then
  Create Server, then the first server card.
- "Show me {server}" opens that server; its console shows the second part (console, power buttons,
  address, sidebar, Settings, logo). Finish it; reloading shows neither part again.
- The menu under your name: "Replay welcome tour" starts the first part again on the server list.
- "Skip tour" on the first part: neither part comes back after a reload.
- An account without servers: the steps end with the empty list and "Create my first server",
  which opens the create page.
- Raise Tour Version and save: the tour shows again for an account that had finished it.
- Narrow the window below 768 px: the card sits at the bottom and steps for the hidden sidebar show
  without a lit element.
