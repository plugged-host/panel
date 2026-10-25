# Welcome tour

A short guide drawn over the real panel, the Pelican panel's (plugged-host-core's
`panel-tour-overlay.blade.php`, same steps and words): the page is dimmed, the part a step is about
stays lit with an accent ring, and the step's card sits next to it. Someone told "open Settings"
afterwards knows where Settings is. The overlay takes every click, so the page cannot be used
mid-tour and the person is always on the page the step talks about. Off by default;
Admin › Settings › Accounts › Welcome Tour turns it on.

## The two parts

The tour runs on the pages it explains, so it has two parts:

| Part | Where | Steps |
|---|---|---|
| Panel | the server list (`/`, `/all`, `/grouped`) | welcome, the sidebar's rows (a bullet per row it shows, named as it is there), the usage card (when the sidebar shows it; bullets for credits and tokens only where those are on), the account row, then Create Server (when it is on) and the first server card, or (no server yet) the empty list and Create Server |
| Server | the next console the user opens after the first part | console, power buttons, name, status and address, the setup guide's callout (a Minecraft server whose setup waits), the server's sidebar (Plugins & Mods, Worlds and Players and Game Settings bullets on Minecraft servers; bullets only for pages the user may open), Settings (when the user may open it), the logo back home |

Step 1 is the acknowledgement: its only button is "Show me around", with no way out. After it,
every step has Back and "Skip tour" at the start and the one filled button at the end (Next, or the
last step's). The first part ends with "Show me {server}" (opens the first server, where the second
part starts), "Create my first server" (opens `/create`) or "Finish"; beside the first two, "Not now"
ends the first part where it is (the second still comes on the next console). "Skip tour" on the
first part ends both parts; on the second part it ends that part, like its "Finish". Escape is
"Skip tour" (not on the acknowledgement).

The card sits beside the lit element (right, below, left or above, whichever fits), with a dot per
step and "Step n of m" above the text. A step whose element is not on screen (a narrow window hides
the sidebar; the tour does not open the phone menu) and the welcome step show a wider card in the
middle of the window. Under 640 px the card is a sheet across the top or bottom, on the side the
element is not on.

"Replay welcome tour" in the menu under the user's name starts both parts again.

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
  starts with "Welcome to {app}" and only "Show me around"; Escape does nothing there. Next lights
  the sidebar's rows (one bullet per row, with its name), then the usage card, then the account
  row at its foot, then Create Server, then the first server card. Clicking the page behind the
  card does nothing.
- "Show me {server}" opens that server; its console shows the second part (console, power buttons,
  name, status and address, the setup callout on a new Minecraft server, sidebar, Settings, logo).
  Finish it; reloading shows neither part again.
- "Not now" on the first part's last step: the list stays, and the next console opened shows the
  second part.
- The menu under your name: "Replay welcome tour" starts the first part again on the server list.
- "Skip tour" (or Escape) on the first part after step 1: neither part comes back after a reload.
- An account without servers: the steps end with the empty list and "Create my first server",
  which opens the create page.
- Raise Tour Version and save: the tour shows again for an account that had finished it.
- Narrow the window below 1024 px: steps for the hidden sidebar show a centred card without a lit
  element; below 640 px the card for a lit element is a sheet at the top or bottom.
