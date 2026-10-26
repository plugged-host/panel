//! plugged.host: start gates, what may hold a server start back before Wings hears about it.
//!
//! Every start and restart a user asks for goes through `POST /api/client/servers/{server}/power`,
//! which asks the gates in order. The first gate that holds the start owns it: the API answers
//! `202 {gate, url}` and the browser goes to that page (a restore, a queue, a waiting room), which
//! sends the start itself once its condition is met. The gates, in order:
//!
//! | Order | Gate | Holds a start while |
//! |---|---|---|
//! | 10 | `limbo` | the server is parked and has to be restored first |
//! | 15 | `setup` | a Minecraft server waits for its setup, which installs and starts it |
//! | 20 | `queue` | its node is too busy: the start waits in the start queue |
//! | 30 | `ads` | an ad-funded waiting room comes first |
//!
//! It fails open: a gate that errors is logged and skipped. A bug in a waiting room must never be
//! why someone cannot start a server.
//!
//! The other ways to start a server are closed for everyone but admins, so the gates cannot be
//! walked around: the websocket token carries no `control.start` / `control.restart`
//! ([`without_start`], used by `Server::wings_permissions`), schedules may not contain a start step
//! ([`schedule_step_starts`]), and a reinstall never starts the server afterwards.

use crate::models::{server::Server, user::User};

/// What a gate decided about one start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The start may go on to the next gate.
    Pass,
    /// The gate owns the start: the user goes to `url`.
    Hold { url: String },
}

/// A gate that may hold a start back (see the module docs).
#[async_trait::async_trait]
pub trait StartGate: Send + Sync {
    /// The gate's name, in the API's answer and the logs (`limbo`, `queue`, `ads`).
    fn name(&self) -> &'static str;

    async fn check(
        &self,
        state: &crate::State,
        server: &Server,
        user: &User,
        action: wings_api::ServerPowerAction,
    ) -> Result<Decision, anyhow::Error>;
}

/// The gates in the order they are asked.
fn gates() -> &'static [&'static dyn StartGate] {
    &[
        &crate::limbo::LimboGate,
        &crate::minecraft::onboarding::SetupGate,
        &crate::start_queue::QueueGate,
        &crate::waiting_room::RoomGate,
    ]
}

/// A start a gate holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub gate: &'static str,
    pub url: String,
}

/// Whether the power action brings the container up: a start, or a restart (which stops it and
/// starts it again, or only starts it when it is offline). Stop and kill are never gated.
#[inline]
pub fn gated(action: wings_api::ServerPowerAction) -> bool {
    matches!(
        action,
        wings_api::ServerPowerAction::Start | wings_api::ServerPowerAction::Restart
    )
}

/// Asks the gates about this start; `Some` when one holds it.
pub async fn run(
    state: &crate::State,
    server: &Server,
    user: &User,
    action: wings_api::ServerPowerAction,
) -> Option<Held> {
    if !gated(action) {
        return None;
    }

    first_hold(
        gates(),
        |gate| gate.name(),
        |gate| gate.check(state, server, user, action),
    )
    .await
}

/// The first gate that holds, asking them in order and none after it; errors are skipped.
async fn first_hold<'a, G: ?Sized + 'a, Fut>(
    gates: impl IntoIterator<Item = &'a &'a G>,
    name: impl Fn(&G) -> &'static str,
    check: impl Fn(&'a G) -> Fut,
) -> Option<Held>
where
    Fut: Future<Output = Result<Decision, anyhow::Error>>,
{
    for gate in gates {
        match check(gate).await {
            Ok(Decision::Pass) => {}
            Ok(Decision::Hold { url }) => {
                return Some(Held {
                    gate: name(gate),
                    url,
                });
            }
            Err(err) => {
                tracing::warn!(
                    gate = name(gate),
                    "start gate failed, letting the start through: {err:#}"
                );
            }
        }
    }

    None
}

/// Every server permission Wings checks, as the panel names them (Wings accepts these as aliases
/// of its own `Permission` names), apart from the admin ones and `*`.
pub const WINGS_SERVER_PERMISSIONS: &[&str] = &[
    "meta.calagopus",
    "websocket.connect",
    "control.read-console",
    "control.console",
    "control.start",
    "control.stop",
    "control.restart",
    "backups.read",
    "schedules.read",
    "files.read",
    "files.read-content",
    "files.create",
    "files.update",
    "files.delete",
    "files.archive",
    "files.sftp",
];

/// The permissions that start a container from the websocket.
const START_PERMISSIONS: &[&str] = &["control.start", "control.restart"];

/// Takes start and restart out of the permissions Wings gets for a user who is not an admin, so
/// those only go through the power API and its gates. `*` becomes the list it stands for.
pub fn without_start(permissions: &mut Vec<&str>) {
    if let Some(star) = permissions.iter().position(|permission| *permission == "*") {
        permissions.remove(star);

        for permission in WINGS_SERVER_PERMISSIONS {
            if !permissions.contains(permission) {
                permissions.push(permission);
            }
        }
    }

    permissions.retain(|permission| !START_PERMISSIONS.contains(permission));
}

/// Whether a schedule step would start the server, which only admins may schedule: a schedule runs
/// on Wings, past the gates. A restart step is allowed; the Wings fork lets a scheduled restart
/// restart a running server only, never start a stopped one.
pub fn schedule_step_starts(action: &wings_api::ScheduleActionInner) -> bool {
    matches!(
        action,
        wings_api::ScheduleActionInner::SendPower {
            action: wings_api::ServerPowerAction::Start,
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Gate(&'static str, Option<Result<Decision, &'static str>>);

    async fn check(
        gate: &Gate,
        asked: &std::sync::Mutex<Vec<&'static str>>,
    ) -> Result<Decision, anyhow::Error> {
        asked.lock().unwrap().push(gate.0);

        match &gate.1 {
            Some(Ok(decision)) => Ok(decision.clone()),
            Some(Err(err)) => Err(anyhow::anyhow!(*err)),
            None => Ok(Decision::Pass),
        }
    }

    fn hold(url: &str) -> Option<Result<Decision, &'static str>> {
        Some(Ok(Decision::Hold { url: url.into() }))
    }

    #[tokio::test]
    async fn the_first_holding_gate_owns_the_start_and_later_ones_are_not_asked() {
        let gates = [
            &Gate("limbo", None),
            &Gate("queue", hold("/server/1/queue")),
            &Gate("ads", hold("/server/1/waiting-room")),
        ];
        let asked = std::sync::Mutex::new(Vec::new());

        let held = first_hold(gates.iter(), |gate| gate.0, |gate| check(gate, &asked)).await;

        assert_eq!(
            held,
            Some(Held {
                gate: "queue",
                url: "/server/1/queue".into()
            })
        );
        assert_eq!(*asked.lock().unwrap(), ["limbo", "queue"]);
    }

    #[tokio::test]
    async fn a_failing_gate_is_skipped() {
        let gates = [
            &Gate("limbo", Some(Err("database gone"))),
            &Gate("queue", None),
        ];
        let asked = std::sync::Mutex::new(Vec::new());

        let held = first_hold(gates.iter(), |gate| gate.0, |gate| check(gate, &asked)).await;

        assert_eq!(held, None);
        assert_eq!(*asked.lock().unwrap(), ["limbo", "queue"]);
    }

    #[test]
    fn only_start_and_restart_are_gated() {
        assert!(gated(wings_api::ServerPowerAction::Start));
        assert!(gated(wings_api::ServerPowerAction::Restart));
        assert!(!gated(wings_api::ServerPowerAction::Stop));
        assert!(!gated(wings_api::ServerPowerAction::Kill));
    }

    #[test]
    fn star_expands_without_start_and_restart() {
        let mut permissions = vec!["websocket.connect", "*", "admin.websocket.install"];
        without_start(&mut permissions);

        assert!(!permissions.contains(&"*"));
        assert!(!permissions.contains(&"control.start"));
        assert!(!permissions.contains(&"control.restart"));
        assert!(permissions.contains(&"control.stop"));
        assert!(permissions.contains(&"files.sftp"));
        assert!(permissions.contains(&"admin.websocket.install"));
        assert_eq!(
            permissions
                .iter()
                .filter(|p| **p == "websocket.connect")
                .count(),
            1
        );
    }

    #[test]
    fn granted_lists_lose_only_start_and_restart() {
        let mut permissions = vec![
            "control.console",
            "control.start",
            "control.restart",
            "files.read",
        ];
        without_start(&mut permissions);

        assert_eq!(permissions, ["control.console", "files.read"]);
    }

    /// Wings' `Permission` enum (plugged-host/wings `application/src/server/permissions.rs`) as of
    /// wings-rs 1.2.x, without `*` and the admin ones. When Wings adds a permission, add it to
    /// [`WINGS_SERVER_PERMISSIONS`] and here, or owners lose it once `*` is expanded.
    #[test]
    fn the_expansion_covers_every_wings_permission() {
        let wings = [
            "meta.calagopus",
            "websocket.connect",
            "control.read-console",
            "control.console",
            "control.start",
            "control.stop",
            "control.restart",
            "backup.read",
            "schedule.read",
            "file.read",
            "file.read-content",
            "file.create",
            "file.update",
            "file.delete",
            "file.archive",
            "file.sftp",
        ];
        // Wings accepts the panel's plural names as aliases of its singular ones.
        let aliased = |permission: &str| {
            permission
                .replacen("backups.", "backup.", 1)
                .replacen("schedules.", "schedule.", 1)
                .replacen("files.", "file.", 1)
        };

        let mut ours: Vec<String> = WINGS_SERVER_PERMISSIONS
            .iter()
            .map(|p| aliased(p))
            .collect();
        ours.sort();
        let mut theirs: Vec<String> = wings.iter().map(|p| p.to_string()).collect();
        theirs.sort();

        assert_eq!(ours, theirs);
    }

    #[test]
    fn only_start_steps_are_refused() {
        let step = |action| wings_api::ScheduleActionInner::SendPower {
            ignore_failure: false,
            action,
        };

        assert!(schedule_step_starts(&step(
            wings_api::ServerPowerAction::Start
        )));
        assert!(!schedule_step_starts(&step(
            wings_api::ServerPowerAction::Restart
        )));
        assert!(!schedule_step_starts(&step(
            wings_api::ServerPowerAction::Stop
        )));
    }
}
