//! plugged.host: reading and writing a Minecraft server's own text files through Wings
//! (docs/features/minecraft.md).

use crate::models::server::Server;
use tokio::io::AsyncReadExt;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// The server's Wings client, refusing the paths a subuser may not see.
pub async fn client(
    state: &crate::State,
    server: &Server,
) -> Result<wings_api::client::WingsClient, anyhow::Error> {
    Ok(server
        .node
        .fetch_cached(&state.database)
        .await?
        .api_client(&state.database)
        .await?
        .ignoring(server.subuser_ignored_files.clone().unwrap_or_default()))
}

/// A text file's contents, `None` when it does not exist. Other failures (Wings down, a file too
/// large) are errors: "not there" must not be mistaken for "could not be read".
pub async fn read_text(
    client: &wings_api::client::WingsClient,
    server: &Server,
    path: &str,
    limit: u64,
) -> Result<Option<String>, anyhow::Error> {
    tokio::time::timeout(TIMEOUT, async {
        let reader = match client
            .get_servers_server_files_contents(
                server.uuid,
                &wings_api::servers_server_files_contents::get::Query {
                    file: Some(format!("/{path}").into()),
                    max_size: Some(limit),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(reader) => reader,
            Err(wings_api::client::ApiHttpError::Http(status, _))
                if status == reqwest::StatusCode::NOT_FOUND =>
            {
                return Ok(None);
            }
            Err(err) => return Err(anyhow::Error::from(err)),
        };

        let mut content = Vec::new();
        reader.take(limit).read_to_end(&mut content).await?;

        Ok(Some(String::from_utf8_lossy(&content).into_owned()))
    })
    .await
    .map_err(|_| anyhow::anyhow!("reading {path} timed out"))?
}

/// Writes a text file, creating it when it is not there.
pub async fn write_text(
    client: &wings_api::client::WingsClient,
    server: &Server,
    path: &str,
    contents: String,
) -> Result<(), anyhow::Error> {
    client
        .post_servers_server_files_write(
            server.uuid,
            wings_api::client::AsyncRequestReader::new(std::io::Cursor::new(contents.into_bytes())),
            &wings_api::servers_server_files_write::post::Query {
                file: Some(format!("/{path}").into()),
                ..Default::default()
            },
        )
        .await?;

    Ok(())
}

/// The server's state on Wings; `None` when Wings does not answer.
pub async fn power_state(
    client: &wings_api::client::WingsClient,
    server: &Server,
) -> Option<wings_api::ServerState> {
    client
        .get_servers_server(server.uuid)
        .await
        .ok()
        .map(|server| server.state)
}

/// Sends one console command. Line breaks would make it two, so a command carrying one is refused.
pub async fn command(
    client: &wings_api::client::WingsClient,
    server: &Server,
    command: &str,
) -> Result<(), anyhow::Error> {
    if command.chars().any(|c| c.is_control()) {
        anyhow::bail!("a console command must be one line");
    }

    client
        .post_servers_server_commands(
            server.uuid,
            &wings_api::servers_server_commands::post::RequestBody {
                commands: vec![command.into()],
            },
        )
        .await?;

    Ok(())
}
