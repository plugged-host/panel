# The app's AFK mining page

The desktop app cannot use Linkvertise, CPX Research or the shorteners: those are browser redirects.
Its Tokens page blurs them and points to the panel instead, and offers **AFK Mining** as the native
way to earn.

The app keeps a local session while it is open. It claims once a minute; the panel pays whole
tokens for the time since the last `mining` row in `user_token_transactions`, but never more than
two minutes per claim, so closing the app stops the clock. Nothing is earned offline.

## The real rig

The page is not an animation. A hardware slider chooses how much of the PC to give, and the app
runs bundled miners only after an explicit **Start mining** click:

- **CPU:** XMRig 6.26.0 mines Monero (RandomX) to the configured XMR wallet through
  `pool.supportxmr.com:3333`. The slider maps to thread count and XMRig CPU priority.
- **GPU:** lolMiner 1.98a mines Ravencoin (KawPow) to the configured RVN wallet through
  `rvn.2miners.com:6060`, once an RVN address is set. At ≤20% the GPU is passed `--devices none`.
- XMRig is embedded in `plugged.host.exe` and extracted to the app's config folder on first Start.
  lolMiner is **not** redistributed (its licence forbids it outside a mining OS): on the first Start
  with an RVN address, the app downloads the official 1.98a Win64 release, verifies its SHA-256 and
  extracts `lolMiner.exe`, `msvcp140.dll` and its licence. XMRig runs without its kernel driver (no
  MSR/huge pages).
- The app polls XMRig's local HTTP API for hashrate/shares; lolMiner's process state is shown even
  when its API does not answer. Stop, logout, session expiry and normal app exit kill both miners.
- Defaults live in `StoredConfig`: `mining_percent`, `mining_xmr_wallet`, `mining_rvn_wallet`,
  `mining_xmr_pool`, `mining_rvn_pool`. The default XMR address is the project's published one; the
  RVN address is empty until the user adds one.

Mining is opt-in and visible. Never enable it at startup, hide the wallets, or add persistence. A
forced process kill can orphan a miner; a future improvement is a Windows Job Object with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.

| Route | What |
|---|---|
| `GET /api/client/account/mining` | Balance, rate, daily cap, last claim and the next claim |
| `POST /api/client/account/mining/start` | Writes a zero-token baseline row and returns the status |
| `POST /api/client/account/mining/claim` | Pays the elapsed whole tokens and returns the status |

`PLUGGED_MINING_TOKENS_PER_HOUR` (default 6) and `PLUGGED_MINING_DAILY_CAP` (default 24) tune it.
The ledger type is `mining`, so the Tokens page's history shows it like every other earning. The
endpoint is available to every signed-in account; the app itself is admin-only on dev for now.
