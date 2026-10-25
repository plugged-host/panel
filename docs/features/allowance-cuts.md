# Allowance cuts

When an account's allowance goes down, its servers give the difference back: what they use beyond
the new allowance, at most what was taken away, comes off all of the account's servers evenly
(`shared/src/allowance_cuts.rs`, table `user_allowance_cuts`). An account that was over before keeps
that part; an account with room to spare loses nothing.

## When

| Where the allowance went down | Recorded by |
|---|---|
| noprofit.space: the account's plan is smaller (a lower tier, a Discord server left, an admin there) | `noprofit::Link::store`, when the account's snapshot is taken on (sign-in, the every-minute sync). A plan with `cut_*` (a pool shared with other services) takes only their growth, never the share shrinking because another service uses more (`allowance_cuts::plan_decrease`) |
| Admin › Settings › Hosting › Games: CPU, memory or disk per account lowered (without noprofit.space) | the settings route, one row for every account that owns a server |

A limit going from a value to none (0) takes nothing; from none to a value takes whatever is over.
A plan that disappears takes nothing (without a plan an account cannot create or grow servers, and
keeps what it has).

## How

The `settle_allowance_cuts` task (every minute, `backend/src/tasks.rs`) carries out every account's
open rows together, under the account's create/resize lock (`allowance::{user}`), so a create or
resize at the same time sees the new sizes:

1. **Asked**: the open rows' sum, per resource.
2. **Taken**: what the account's servers use beyond the allowance now, at most what was asked.
3. Spread evenly over the account's servers (two servers half each, five a fifth each); a server
   that would drop below its floor gives what it can and the others share the rest:

   | Resource | Floor |
   |---|---|
   | CPU | 25% |
   | Memory | 1 GiB |
   | Disk | 1 GiB, and never below what the server stores now plus 1 GiB (Wings stops a server over its disk); a server whose node does not answer gives no disk |

   Servers without a limit on a resource give none of it.
4. The new limits are saved and sent to Wings, which applies them to the running containers.
5. Each server's activity log gets `server:allowance.cut` (`{reason, cpu, memory, disk}`); the rows
   are closed with what came off which server (`result`).

What no server can give stays untaken; new servers and resizes are refused until the account fits
again ([create-server.md](create-server.md#allowance)).

## Without noprofit.space

The games settings' allowance is the one that goes down (the second row above).

## Check

- Without noprofit.space, Admin › Settings › Hosting › Games: allowance 8 GiB memory; an account
  with two servers of 4 GiB each. Lower the allowance to 6 GiB and save: within a minute each server
  has 3 GiB (Admin › Servers), and both servers' Activity shows `server:allowance.cut`.
- An account that uses less than the new allowance keeps its servers as they are.
- With noprofit.space: lower an account's plan memory there; after the next sync (every minute) its
  servers are brought down the same way.
