// plugged.host: devboxes (docs/features/devboxes.md).
use shared::extensions::commands::CliCommandGroupBuilder;

mod edge_certificates;

pub fn commands(cli: CliCommandGroupBuilder) -> CliCommandGroupBuilder {
    cli.add_command(
        "edge-certificates",
        "Writes the certificates the browser edge for devboxes needs.",
        edge_certificates::EdgeCertificatesCommand,
    )
}
