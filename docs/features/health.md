# Health checks

Admin › Overview › Health is upstream's page (migrations, NTP offset, debug mode, node clocks). The
fork adds two cards, so a stopped scheduler or sampler shows up there instead of in a `doctor`
command:

| Card | Shows | API |
|---|---|---|
| Background Tasks | every background task (`backend/src/tasks.rs`) with when it last started and the error of its last run; tasks run on the primary instance only, other instances say so | `GET /api/admin/system/health/tasks` |
| Node Telemetry | nodes whose [telemetry](node-telemetry.md) sample is missing or older than 150 seconds (the sampler runs every minute), linked to the node | `GET /api/admin/system/health/telemetry` |

Both need `stats.read`; the telemetry card also `nodes.read`. A task's time is when it last started
(the panel's start, for one that has not run yet), so a per-minute task more than a few minutes old,
or a three-hourly one more than three hours old, has stopped or is stuck in a long run.
