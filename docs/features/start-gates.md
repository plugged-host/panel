# Start gates

A server start can be held back before Wings hears about it: a parked server has to be restored
first, a new Minecraft server waits for its setup, a busy node makes the start wait in a queue, or
an ad-funded waiting room comes first. The
start gates are the one place where those features step in, so none of them can be walked around and
none of them needs to know about the others.

## How a start flows

1. Every start or restart a user asks for goes to `POST /api/client/servers/{server}/power` (the
   console's power buttons, the status card, quick actions, the EULA and Java-version prompts, the
   server list's row menu and bulk actions). The server's pages use `usePowerAction()`
   (`frontend/src/plugins/server/usePowerAction.ts`), where stop and kill still go over the
   websocket; the server list uses `useBulkPowerActions()`.
2. The route checks the permission, then asks the gates in order (`shared/src/start_gates.rs`):

   | Order | Gate | Holds a start while | Phase |
   |---|---|---|---|
   | 10 | `limbo` | the server is parked and has to be restored first | 3 |
   | 15 | `setup` | a Minecraft server waits for its setup, which installs and starts it ([minecraft.md](minecraft.md#setup)); the user goes to the setup page | 4 |
   | 20 | `queue` | its node is too busy: the start waits in the start queue | 3 |
   | 30 | `ads` | an ad-funded waiting room comes first | 3 |

3. The first gate that holds the start owns it: the API answers `202 {"gate": "queue", "url":
   "/server/…/queue"}` and the browser goes to that page, which sends the start itself once its
   condition is met. No gate after it is asked. Without a hold the start goes to Wings as usual
   (`200 {}`). From the server list, one server goes to the gate's page as well; a bulk start counts
   the held servers apart ("2 servers wait before starting") instead of calling them started.
4. A gate that fails is logged and skipped: a bug in a waiting room must never be why someone cannot
   start a server.

Limbo ([limbo.md](limbo.md)), the Minecraft setup ([minecraft.md](minecraft.md#setup), its
`SetupGate` in `shared/src/minecraft/onboarding.rs`), the start queue
([start-queue.md](start-queue.md)) and the ad waiting room ([ads.md](ads.md#waiting-room)) are in.
The setup starts the server itself through Wings once it is done, so it never asks the gates. The waiting room sends the start to Wings
itself on the click, past the gates, and only holds the user on its page.

## Closing the other ways to start a server

For everyone but admins:

| Way | How it is closed |
|---|---|
| The websocket's `set state start/restart` | The websocket token (and the permissions Wings fetches at an SFTP login) carry no `control.start` / `control.restart`; `*` is expanded into Wings' permission list without them (`start_gates::without_start`, called by `Server::wings_permissions`). Wings refuses the message |
| A schedule step that starts the server | Refused when a step is created, changed or imported ("a schedule cannot start the server"). A restart step is allowed: the Wings fork only restarts a running server from a schedule ([wings.md](../wings.md)) |
| "Start after reinstall" | The option is hidden and the API ignores it |

Admins keep every path; the gates themselves decide whether they hold an admin's start.

## Adding a gate

Implement `StartGate` (`name()` and `check()` returning `Decision::Pass` or `Decision::Hold { url
}`) next to the feature's other code, and add it to `gates()` in `shared/src/start_gates.rs` at its
place in the order. Its page sends the start with `POST /power` once it lets it through, so it must
not hold that start again (record that it let it through, for example), or it sends the power
action to Wings itself, past the gates, as the start queue does.

## Check

- As a non-admin owner: Start, Restart and Stop from the console work; the console shows no error.
- Sending `{"event":"set state","args":["start"]}` over the server's websocket by hand is refused
  (Wings answers with a permission error); stop still works that way.
- Creating a schedule step "Send power: start" is refused with "a schedule cannot start the
  server"; "restart" is accepted. As an admin both are accepted.
- The reinstall dialog has no "start after reinstall" switch for a non-admin.
- A Minecraft server still waiting for its setup: Start from the server list (or the API) goes to
  the setup page (`202 {"gate": "setup"}`); after Finish setup the setup starts it.
