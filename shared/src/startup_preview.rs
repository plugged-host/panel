// plugged.host: the server's startup command with its values filled in, the Pelican panel's
// "Enable Preview" on the startup command (StartupCommandService) (docs/features/server-settings.md#startup).

use crate::models::{server::Server, server_variable::ServerVariable};
use std::collections::HashMap;

/// What a variable the viewer may not read shows instead of its value, as on the Pelican panel.
pub const HIDDEN: &str = "[hidden]";

/// One egg variable for [`values`]: its name, its value (the server's, else the egg's default)
/// and whether the viewer may read it.
pub struct Variable<'a> {
    pub env_variable: &'a str,
    pub value: &'a str,
    pub readable: bool,
}

/// What each `{{NAME}}` stands for: `SERVER_MEMORY`, `SERVER_IP` and `SERVER_PORT` as Wings sets
/// them (the Pelican panel's 127.0.0.1 and 0 without an allocation), then the egg's variables,
/// which win over those as they do in Wings' environment. `None` is a value the viewer may not read.
pub fn values<'a>(
    memory: i64,
    allocation: Option<(String, i32)>,
    variables: impl IntoIterator<Item = Variable<'a>>,
) -> HashMap<String, Option<String>> {
    let (ip, port) = allocation.unwrap_or_else(|| ("127.0.0.1".into(), 0));

    let mut values = HashMap::from([
        ("SERVER_MEMORY".to_string(), Some(memory.to_string())),
        ("SERVER_IP".to_string(), Some(ip)),
        ("SERVER_PORT".to_string(), Some(port.to_string())),
    ]);

    for variable in variables {
        values.insert(
            variable.env_variable.to_string(),
            variable.readable.then(|| variable.value.to_string()),
        );
    }

    values
}

/// `startup` with every `{{NAME}}` of `values` replaced, in one pass, so a value never has its
/// own `{{…}}` filled in. Names it does not know stay as they are.
pub fn render(startup: &str, values: &HashMap<String, Option<String>>) -> String {
    let mut rendered = String::with_capacity(startup.len());
    let mut rest = startup;

    while let Some(start) = rest.find("{{") {
        rendered.push_str(&rest[..start]);
        let after = &rest[start + 2..];

        match after
            .find("}}")
            .and_then(|end| Some((end, values.get(&after[..end])?)))
        {
            Some((end, value)) => {
                rendered.push_str(value.as_deref().unwrap_or(HIDDEN));
                rest = &after[end + 2..];
            }
            // not a name it knows: keep the braces and look again right after them
            None => {
                rendered.push_str("{{");
                rest = after;
            }
        }
    }

    rendered.push_str(rest);
    rendered
}

/// The preview of `server`'s startup command. Variables the egg hides from players and secret
/// ones show as [`HIDDEN`].
pub async fn for_server(state: &crate::State, server: &Server) -> Result<String, anyhow::Error> {
    for_command(state, server, &server.startup).await
}

/// [`for_server`] for another command on `server`: a custom one being written
/// (docs/features/startup-commands.md).
pub async fn for_command(
    state: &crate::State,
    server: &Server,
    command: &str,
) -> Result<String, anyhow::Error> {
    let variables =
        ServerVariable::all_by_server_uuid_egg_uuid(&state.database, server.uuid, server.egg.uuid)
            .await?;

    let values = values(
        server.memory,
        server.allocation.as_ref().map(|allocation| {
            (
                allocation.allocation.ip.ip().to_string(),
                allocation.allocation.port,
            )
        }),
        variables.iter().map(|variable| Variable {
            env_variable: &variable.variable.env_variable,
            value: &variable.value,
            readable: variable.variable.user_viewable && !variable.variable.secret,
        }),
    );

    Ok(render(command, &values))
}

#[cfg(test)]
mod tests {
    use super::{HIDDEN, Variable, render, values};

    fn variable<'a>(env_variable: &'a str, value: &'a str, readable: bool) -> Variable<'a> {
        Variable {
            env_variable,
            value,
            readable,
        }
    }

    #[test]
    fn the_server_values_are_filled_in() {
        let values = values(2048, Some(("192.0.2.10".into(), 25565)), []);

        assert_eq!(
            render(
                "java -Xmx{{SERVER_MEMORY}}M -jar server.jar --host {{SERVER_IP}} --port {{SERVER_PORT}}",
                &values
            ),
            "java -Xmx2048M -jar server.jar --host 192.0.2.10 --port 25565"
        );
    }

    #[test]
    fn without_an_allocation_it_shows_what_the_pelican_panel_shows() {
        let values = values(512, None, []);

        assert_eq!(
            render("{{SERVER_IP}}:{{SERVER_PORT}}", &values),
            "127.0.0.1:0"
        );
    }

    #[test]
    fn variables_the_viewer_may_not_read_are_hidden() {
        let values = values(
            1024,
            None,
            [
                variable("SERVER_JARFILE", "paper.jar", true),
                variable("RCON_PASSWORD", "hunter2", false),
            ],
        );

        assert_eq!(
            render(
                "java -jar {{SERVER_JARFILE}} --rcon {{RCON_PASSWORD}}",
                &values
            ),
            format!("java -jar paper.jar --rcon {HIDDEN}")
        );
    }

    #[test]
    fn a_variable_wins_over_the_server_value_of_its_name() {
        let values = values(1024, None, [variable("SERVER_MEMORY", "900", true)]);

        assert_eq!(render("-Xmx{{SERVER_MEMORY}}M", &values), "-Xmx900M");
    }

    #[test]
    fn unknown_names_and_stray_braces_stay() {
        let values = values(1024, None, [variable("PORT", "1", true)]);

        assert_eq!(
            render("{{UNKNOWN}} {{PORT}} {{ {{{{PORT}} }} {{PORT", &values),
            "{{UNKNOWN}} 1 {{ {{1 }} {{PORT"
        );
        assert_eq!(render("", &values), "");
        assert_eq!(render("no placeholders", &values), "no placeholders");
    }

    #[test]
    fn a_value_is_never_filled_in_itself() {
        let values = values(
            1024,
            None,
            [
                variable("GREETING", "{{SECRET}}", true),
                variable("SECRET", "hunter2", false),
            ],
        );

        assert_eq!(render("{{GREETING}}", &values), "{{SECRET}}");
    }
}
