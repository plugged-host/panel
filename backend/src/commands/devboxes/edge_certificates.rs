// plugged.host: writes the devbox certificate authority and the edge's client certificate to the
// edge's nginx (docs/features/devboxes.md). The edge key never leaves the panel's machines
// through the API: it is only written here.
use clap::{Args, FromArgMatches};
use colored::Colorize;
use std::{io::Write, os::unix::fs::OpenOptionsExt, path::Path};

#[derive(Args)]
pub struct EdgeCertificatesArgs {
    #[arg(
        long = "dir",
        help = "the directory to write ca.crt, edge.crt and edge.key into"
    )]
    dir: String,
}

fn write(path: &Path, contents: &str, mode: u32) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)?;
    file.write_all(contents.as_bytes())?;
    // an existing file keeps its mode on open: set it again
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(mode))
}

pub struct EdgeCertificatesCommand;

impl shared::extensions::commands::CliCommand<EdgeCertificatesArgs> for EdgeCertificatesCommand {
    fn get_command(&self, command: clap::Command) -> clap::Command {
        command
    }

    fn get_executor(self) -> Box<shared::extensions::commands::ExecutorFunc> {
        Box::new(|env, arg_matches| {
            Box::pin(async move {
                let args = EdgeCertificatesArgs::from_arg_matches(&arg_matches)?;
                let state = shared::AppState::new_cli(env).await?;
                let settings = state.settings.get().await?;

                let (Some(ca_cert), Some(edge_cert), Some(edge_key)) = (
                    settings.devboxes.ca_cert.clone(),
                    settings.devboxes.edge_cert.clone(),
                    settings.devboxes.edge_key.clone(),
                ) else {
                    eprintln!(
                        "{}",
                        "no devbox certificates yet: create them in Admin › Settings › Hosting › Devboxes"
                            .red()
                    );
                    return Ok(1);
                };
                drop(settings);

                let dir = Path::new(&args.dir);
                std::fs::create_dir_all(dir)?;
                write(&dir.join("ca.crt"), &ca_cert, 0o644)?;
                write(&dir.join("edge.crt"), &edge_cert, 0o644)?;
                write(&dir.join("edge.key"), &edge_key, 0o600)?;

                eprintln!(
                    "wrote ca.crt, edge.crt and edge.key to {}; reload nginx to use them",
                    dir.display().to_string().cyan()
                );

                Ok(0)
            })
        })
    }
}
