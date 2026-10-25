//! plugged.host: neo's tools for a server's files (docs/features/neo.md › Tools). They go through
//! Wings like the file manager's routes, with the subuser's ignored files hidden the same way.

use super::{Tool, ToolContext, ToolError, ToolOutcome, ToolResult, ToolRisk, support};
use crate::models::server::Server;
use serde_json::{Value, json};
use std::borrow::Cow;
use tokio::io::AsyncReadExt;

const MAX_ENTRIES: u64 = 200;
const MAX_READ_LINES: i64 = 400;
/// The most of a file neo reads at once (the file manager's own limit when that is lower).
const MAX_READ_BYTES: u64 = 1024 * 1024;
/// The most `write_file` writes in one go; an edited file may grow to what neo reads at once.
const MAX_WRITE_BYTES: usize = 256 * 1024;
const MAX_SEARCH_RESULTS: u64 = 100;
const MAX_ARCHIVE_NAMES: usize = 50;
const MAX_DELETE: usize = 20;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

const SECRET_REFUSAL: &str = "Masked •••• values cannot be written back by neo; ask the user to change that value themselves.";

fn not_found(path: &str) -> ToolError {
    ToolError::invalid(format!("{path} does not exist."))
}

/// The chat's server as a copy that can check ignored paths (which caches the compiled list).
fn server_copy(context: &ToolContext<'_>) -> Result<Server, ToolError> {
    Ok(context.server()?.clone())
}

/// A file path argument; the server root is not a file.
fn file_path(arguments: &Value, key: &str) -> Result<String, ToolError> {
    let path = support::path(arguments.get(key).unwrap_or(&Value::Null), "/")?;
    if path == "/" {
        return Err(ToolError::invalid(format!(
            "\"{key}\" must be a path below the server root, like /server.properties."
        )));
    }

    Ok(path)
}

/// Escapes what Wings' search reads as glob syntax, so a name is matched as written.
fn escape_glob(pattern: &str) -> String {
    let mut escaped = String::with_capacity(pattern.len());
    for c in pattern.chars() {
        if matches!(c, '\\' | '*' | '?' | '[' | ']' | '{' | '}') {
            escaped.push('\\');
        }
        escaped.push(c);
    }

    escaped
}

/// A file's raw bytes, at most `max`.
async fn read_bytes(
    context: &ToolContext<'_>,
    server: &mut Server,
    path: &str,
    max: u64,
) -> Result<Vec<u8>, ToolError> {
    if server.is_ignored(path, false) {
        return Err(not_found(path));
    }

    let client = support::wings(context).await?;
    let content = tokio::time::timeout(TIMEOUT, async {
        let reader = client
            .get_servers_server_files_contents(
                server.uuid,
                &wings_api::servers_server_files_contents::get::Query {
                    file: Some(path.into()),
                    max_size: Some(max),
                    ignored: server.subuser_ignored_files.clone(),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| match err {
                wings_api::client::ApiHttpError::Http(status, _)
                    if status == reqwest::StatusCode::NOT_FOUND =>
                {
                    not_found(path)
                }
                wings_api::client::ApiHttpError::Http(status, _)
                    if status == reqwest::StatusCode::PAYLOAD_TOO_LARGE =>
                {
                    ToolError::invalid(format!(
                        "{path} is too large to read here (over {}).",
                        support::human(max)
                    ))
                }
                err => support::wings_error(err),
            })?;

        let mut content = Vec::new();
        reader
            .take(max)
            .read_to_end(&mut content)
            .await
            .map_err(|err| ToolError::Failed(err.into()))?;

        Ok::<_, ToolError>(content)
    })
    .await
    .map_err(|_| ToolError::invalid(format!("Reading {path} took too long.")))??;

    if content.contains(&0) {
        return Err(ToolError::invalid(format!(
            "{path} is a binary file, not text."
        )));
    }

    Ok(content)
}

/// The most neo reads of one file: its own cap, or the file manager's when that is lower.
async fn read_limit(state: &crate::State) -> Result<u64, ToolError> {
    let view = state
        .settings
        .get_as(|settings| settings.server.max_file_manager_view_size)
        .await?;

    Ok(if view == 0 {
        MAX_READ_BYTES
    } else {
        view.min(MAX_READ_BYTES)
    })
}

async fn write_text(
    context: &ToolContext<'_>,
    server: &Server,
    path: &str,
    content: String,
) -> Result<(), ToolError> {
    support::wings(context)
        .await?
        .post_servers_server_files_write(
            server.uuid,
            wings_api::client::AsyncRequestReader::new(std::io::Cursor::new(content.into_bytes())),
            &wings_api::servers_server_files_write::post::Query {
                file: Some(path.into()),
                user: Some(context.user.uuid),
                ignored: server.subuser_ignored_files.clone(),
                ..Default::default()
            },
        )
        .await
        .map_err(support::wings_error)?;

    Ok(())
}

/// Lines `start..start+count` (1-based) of the text with right-aligned numbers, and how many lines
/// the text has.
fn numbered(text: &str, start: usize, count: usize) -> (Vec<String>, usize) {
    let lines: Vec<&str> = text.split('\n').collect();
    let total = lines.len();
    let slice: Vec<&str> = lines.into_iter().skip(start - 1).take(count).collect();
    let width = (start + slice.len()).to_string().len();

    (
        slice
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>width$}| {line}", start + i))
            .collect(),
        total,
    )
}

/// Replaces `old` in `content`: every occurrence with `all`, else exactly one. The count, or why not.
fn replace(content: &str, old: &str, new: &str, all: bool) -> Result<(String, usize), String> {
    let count = content.matches(old).count();
    if count == 0 {
        return Err("not_found".into());
    }
    if count > 1 && !all {
        return Err(format!("{count}"));
    }

    Ok(if all {
        (content.replace(old, new), count)
    } else {
        (content.replacen(old, new, 1), 1)
    })
}

pub struct ListFilesTool;

#[async_trait::async_trait]
impl Tool for ListFilesTool {
    fn name(&self) -> &'static str {
        "list_files"
    }

    fn description(&self) -> Cow<'static, str> {
        "List a directory on the server (folders first, with sizes and last-modified dates). Paths start at the server root, e.g. \"/\" or \"/plugins\".".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Directory to list, default \"/\"."},
            },
        })
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.read"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!("List {}", support::shown_path(arguments, "path"))
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let path = support::path(arguments.get("path").unwrap_or(&Value::Null), "/")?;
        if server.is_ignored_subtree(&path) {
            return Err(not_found(&path));
        }

        let listing = support::wings(context)
            .await?
            .get_servers_server_files_list(
                server.uuid,
                &wings_api::servers_server_files_list::get::Query {
                    directory: Some(path.as_str().into()),
                    ignored: server.subuser_ignored_files.clone(),
                    per_page: Some(MAX_ENTRIES),
                    page: Some(1),
                    sort: Some(wings_api::DirectorySortingMode::NameAsc),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| match err {
                wings_api::client::ApiHttpError::Http(status, _)
                    if status == reqwest::StatusCode::NOT_FOUND =>
                {
                    not_found(&path)
                }
                err => support::wings_error(err),
            })?;

        let mut entries = listing.entries;
        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let mut lines: Vec<String> = entries
            .iter()
            .map(|entry| {
                let modified = entry.modified.format("%Y-%m-%d %H:%M");
                if entry.directory {
                    format!("[dir]  {}/  {modified}", entry.name)
                } else {
                    format!(
                        "[file] {}  {}  {modified}",
                        entry.name,
                        support::human(entry.size)
                    )
                }
            })
            .collect();
        if listing.total > entries.len() as u64 {
            lines.push(format!(
                "…and {} more entries",
                listing.total - entries.len() as u64
            ));
        }

        let content = if lines.is_empty() {
            format!("{path} is empty.")
        } else {
            format!("{path}:\n{}", lines.join("\n"))
        };

        Ok(ToolResult::ok(support::limit_output(&content)).with_display(format!("Listed {path}")))
    }
}

pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> Cow<'static, str> {
        "Read a text file on the server with line numbers (up to 400 lines per call; use start_line to page). Secrets such as passwords and tokens are masked as ••••.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path from the server root, e.g. \"/server.properties\"."},
                "start_line": {"type": "integer", "minimum": 1, "description": "First line to return (default 1)."},
                "max_lines": {"type": "integer", "minimum": 1, "maximum": MAX_READ_LINES, "description": "How many lines (default 400)."},
            },
            "required": ["path"],
        })
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.read-content"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!("Read {}", support::shown_path(arguments, "path"))
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let path = file_path(arguments, "path")?;
        let max = read_limit(context.state).await?;
        let content = read_bytes(context, &mut server, &path, max).await?;

        // masked before numbering: the patterns look for secrets at the start of a line
        let text = support::redact(&String::from_utf8_lossy(&content).replace("\r\n", "\n"));
        let start = support::integer(arguments, "start_line", 1, 1, i64::MAX) as usize;
        let count = support::integer(arguments, "max_lines", MAX_READ_LINES, 1, MAX_READ_LINES);
        let (lines, total) = numbered(&text, start, count as usize);

        if lines.is_empty() && start > 1 {
            return Err(ToolError::invalid(format!(
                "{path} has only {total} lines."
            )));
        }

        context.log("file.read", json!({"file": path})).await;

        let header = format!(
            "{path} (lines {start}-{} of {total})",
            start + lines.len().saturating_sub(1)
        );

        Ok(ToolResult::ok(support::limit_output(&format!(
            "{header}\n{}",
            lines.join("\n")
        )))
        .with_display(format!("Read {path}")))
    }
}

pub struct SearchFilesTool;

#[async_trait::async_trait]
impl Tool for SearchFilesTool {
    fn name(&self) -> &'static str {
        "search_files"
    }

    fn description(&self) -> Cow<'static, str> {
        "Find files and folders whose name contains a pattern (at least 3 characters, any case), searching below a directory.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Part of a file or folder name, e.g. \"latest.log\" or \".yml\"."},
                "directory": {"type": "string", "description": "Where to search, default \"/\"."},
            },
            "required": ["pattern"],
        })
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.read"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Search for \"{}\"",
            support::short(support::shown(arguments, "pattern"), 40)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let pattern = support::string(arguments, "pattern", 100, true)?.trim();
        if pattern.chars().count() < 3 {
            return Err(ToolError::invalid(
                "The pattern must be at least 3 characters.",
            ));
        }

        let directory = support::path(arguments.get("directory").unwrap_or(&Value::Null), "/")?;
        if server.is_ignored_subtree(&directory) {
            return Err(not_found(&directory));
        }

        let per_page = context
            .state
            .settings
            .get_as(|settings| settings.server.max_file_manager_search_results)
            .await?
            .clamp(1, MAX_SEARCH_RESULTS);

        let client = support::wings(context).await?;
        let request = wings_api::servers_server_files_search::post::RequestBody {
            root: directory.as_str().into(),
            path_filter: Some(
                wings_api::servers_server_files_search::post::RequestBodyPathFilter {
                    include: vec![format!("**/*{}*", escape_glob(pattern)).into()],
                    exclude: server.subuser_ignored_files.clone().unwrap_or_default(),
                    case_insensitive: true,
                },
            ),
            size_filter: None,
            content_filter: None,
            per_page,
        };

        let response = match tokio::time::timeout(
            TIMEOUT,
            client.post_servers_server_files_search(server.uuid, &request),
        )
        .await
        {
            Ok(response) => response.map_err(|err| match err {
                wings_api::client::ApiHttpError::Http(status, _)
                    if status == reqwest::StatusCode::NOT_FOUND =>
                {
                    not_found(&directory)
                }
                err => support::wings_error(err),
            })?,
            Err(_) => {
                return Ok(ToolResult::error(
                    "The search took too long; search a smaller directory.",
                ));
            }
        };

        let results: Vec<String> = response
            .results
            .iter()
            .map(|entry| {
                format!(
                    "{} {}",
                    if entry.directory { "[dir] " } else { "[file]" },
                    support::join(&directory, &entry.name)
                )
            })
            .collect();

        Ok(ToolResult::ok(if results.is_empty() {
            "No matches.".to_string()
        } else {
            support::limit_output(&results.join("\n"))
        })
        .with_display(format!("{} matches", results.len())))
    }
}

pub struct EditFileTool;

#[async_trait::async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn description(&self) -> Cow<'static, str> {
        "Replace an exact piece of text in a file. old_text must appear exactly once (copy it from read_file without the line-number prefix), unless replace_all is true. Masked •••• values cannot be edited.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "old_text": {"type": "string", "description": "Exact existing text to replace."},
                "new_text": {"type": "string", "description": "Replacement text."},
                "replace_all": {"type": "boolean", "description": "Replace every occurrence (default false)."},
            },
            "required": ["path", "old_text", "new_text"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn offered_with(&self) -> Vec<&'static str> {
        vec!["files.create"]
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.read-content", "files.create"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!("Edit {}", support::shown_path(arguments, "path"))
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let path = file_path(arguments, "path")?;
        let old = support::string(arguments, "old_text", 50_000, true)?;
        let Some(new) = arguments.get("new_text").and_then(Value::as_str) else {
            return Err(ToolError::invalid("\"new_text\" must be a string."));
        };
        if support::masked(old) || support::masked(new) {
            return Err(ToolError::invalid(SECRET_REFUSAL));
        }

        let max = read_limit(context.state).await?;
        let content = read_bytes(context, &mut server, &path, max).await?;
        let Ok(content) = String::from_utf8(content) else {
            return Err(ToolError::invalid(format!(
                "{path} is not UTF-8 text; it can only be edited in the file manager."
            )));
        };

        let (updated, count) = match replace(
            &content,
            old,
            new,
            support::boolean(arguments, "replace_all", false),
        ) {
            Ok(done) => done,
            Err(reason) if reason == "not_found" => {
                return Err(ToolError::invalid(format!(
                    "old_text was not found in {path}. Read the file again and copy the text exactly."
                )));
            }
            Err(count) => {
                return Err(ToolError::invalid(format!(
                    "old_text appears {count} times in {path}; include more surrounding text or set replace_all."
                )));
            }
        };
        if updated.len() as u64 > max {
            return Err(ToolError::invalid("The edited file would be too large."));
        }

        write_text(context, &server, &path, updated).await?;
        context
            .log("file.edit", json!({"file": path, "replaced": count}))
            .await;

        Ok(
            ToolResult::ok(format!("Edited {path}: replaced {count} occurrence(s)."))
                .with_display(format!("Edited {path}")),
        )
    }
}

pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> Cow<'static, str> {
        "Create a file or replace its whole content. For small changes to an existing file prefer edit_file. Never write the •••• mask back: masked secrets must be changed by the user.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path from the server root."},
                "content": {"type": "string", "description": "The complete new file content."},
            },
            "required": ["path", "content"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.create"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        let size = arguments
            .get("content")
            .and_then(Value::as_str)
            .map_or(0, str::len);

        format!(
            "Write {} ({})",
            support::shown_path(arguments, "path"),
            support::human(size as u64)
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let path = file_path(arguments, "path")?;
        let Some(content) = arguments.get("content").and_then(Value::as_str) else {
            return Err(ToolError::invalid(
                "Give a file path and the content as a string.",
            ));
        };
        if content.len() > MAX_WRITE_BYTES {
            return Err(ToolError::invalid(
                "That content is too large to write in one go.",
            ));
        }
        if support::masked(content) {
            return Err(ToolError::invalid(SECRET_REFUSAL));
        }
        if server.is_ignored(&path, false) {
            return Err(not_found(&path));
        }

        let size = content.len();
        write_text(context, &server, &path, content.to_string()).await?;
        context
            .log("file.write", json!({"file": path, "bytes": size}))
            .await;

        Ok(ToolResult::ok(format!("Wrote {path} ({size} bytes)."))
            .with_display(format!("Wrote {path}")))
    }
}

pub struct CreateDirectoryTool;

#[async_trait::async_trait]
impl Tool for CreateDirectoryTool {
    fn name(&self) -> &'static str {
        "create_directory"
    }

    fn description(&self) -> Cow<'static, str> {
        "Create a folder on the server.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Folder to create, e.g. \"/plugins/MyPlugin\"."},
            },
            "required": ["path"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.create"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!("Create folder {}", support::shown_path(arguments, "path"))
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let path = file_path(arguments, "path")?;
        let (parent, name) = support::split(&path);
        if server.is_ignored_subtree(&parent) {
            return Err(not_found(&parent));
        }
        if server.is_ignored(&path, true) {
            return Err(ToolError::invalid(format!("{path} cannot be created.")));
        }

        support::wings(context)
            .await?
            .post_servers_server_files_create_directory(
                server.uuid,
                &wings_api::servers_server_files_create_directory::post::RequestBody {
                    root: parent.as_str().into(),
                    name: name.as_str().into(),
                },
            )
            .await
            .map_err(support::wings_error)?;
        context
            .log("file.create-directory", json!({"file": path}))
            .await;

        Ok(ToolResult::ok(format!("Created {path}.")).with_display(format!("Created {path}")))
    }
}

pub struct RenameFileTool;

#[async_trait::async_trait]
impl Tool for RenameFileTool {
    fn name(&self) -> &'static str {
        "rename_file"
    }

    fn description(&self) -> Cow<'static, str> {
        "Rename or move a file or folder.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "from": {"type": "string", "description": "Current path."},
                "to": {"type": "string", "description": "New path."},
            },
            "required": ["from", "to"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.update"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!(
            "Move {} → {}",
            support::shown_path(arguments, "from"),
            support::shown_path(arguments, "to")
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let from = file_path(arguments, "from")?;
        let to = file_path(arguments, "to")?;
        if server.is_ignored_subtree(&from) {
            return Err(not_found(&from));
        }
        if server.is_ignored_either(&to) {
            return Err(ToolError::invalid(format!("{to} cannot be written.")));
        }

        let renamed = support::wings(context)
            .await?
            .put_servers_server_files_rename_with(
                server.uuid,
                &wings_api::servers_server_files_rename::put::RequestBody {
                    root: "/".into(),
                    files: vec![
                        wings_api::servers_server_files_rename::put::RequestBodyFiles {
                            from: from.trim_start_matches('/').into(),
                            to: to.trim_start_matches('/').into(),
                        },
                    ],
                },
                &wings_api::servers_server_files_rename::put::Extra {
                    create_directories: context.can("files.create"),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| match err {
                wings_api::client::ApiHttpError::Http(status, _)
                    if status == reqwest::StatusCode::NOT_FOUND =>
                {
                    not_found(&from)
                }
                err => support::wings_error(err),
            })?
            .renamed;

        if renamed == 0 {
            return Err(ToolError::invalid(format!(
                "Nothing was moved: {from} may not exist, or {to} is taken."
            )));
        }

        context
            .log("file.rename", json!({"file": from, "to": to}))
            .await;

        Ok(ToolResult::ok(format!("Moved {from} to {to}.")).with_display(format!("Moved {from}")))
    }
}

pub struct CompressFilesTool;

/// The names to pack: plain names or relative paths inside the directory, never `..`.
fn archive_names(arguments: &Value) -> Result<Vec<String>, ToolError> {
    let names: Vec<String> = support::strings(arguments, "names", MAX_ARCHIVE_NAMES)?
        .into_iter()
        .map(|name| {
            name.trim_matches(|c: char| c == '/' || c.is_whitespace())
                .to_string()
        })
        .filter(|name| !name.is_empty() && !name.split(['/', '\\']).any(|part| part == ".."))
        .collect();

    if names.is_empty() {
        return Err(ToolError::invalid(format!(
            "Give between 1 and {MAX_ARCHIVE_NAMES} names from the directory."
        )));
    }

    Ok(names)
}

#[async_trait::async_trait]
impl Tool for CompressFilesTool {
    fn name(&self) -> &'static str {
        "compress_files"
    }

    fn description(&self) -> Cow<'static, str> {
        "Pack files or folders from one directory into a .tar.gz archive in that directory (useful as a quick copy before a risky change). Big folders are packed in the background.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "directory": {"type": "string", "description": "Directory the files are in, default \"/\"."},
                "names": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_ARCHIVE_NAMES, "description": "File or folder names inside that directory."},
            },
            "required": ["names"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.archive"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        let count = arguments
            .get("names")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);

        format!(
            "Archive {count} item(s) in {}",
            support::shown_path(arguments, "directory")
        )
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let directory = support::path(arguments.get("directory").unwrap_or(&Value::Null), "/")?;
        let names = archive_names(arguments)?;
        if server.is_ignored_subtree(&directory) {
            return Err(not_found(&directory));
        }
        if let Some(hidden) = names
            .iter()
            .find(|name| server.is_ignored_subtree(support::join(&directory, name)))
        {
            return Err(not_found(&support::join(&directory, hidden)));
        }

        let name = format!(
            "archive-{}.tar.gz",
            chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S")
        );
        let archive = support::join(&directory, &name);

        let response = support::wings(context)
            .await?
            .post_servers_server_files_compress(
                server.uuid,
                &wings_api::servers_server_files_compress::post::RequestBody {
                    name: Some(name.as_str().into()),
                    format: wings_api::ArchiveFormat::TarGz,
                    root: directory.as_str().into(),
                    files: names.iter().map(|name| name.as_str().into()).collect(),
                    foreground: false,
                },
            )
            .await
            .map_err(support::wings_error)?;
        context
            .log("file.compress", json!({"file": archive, "files": names}))
            .await;

        Ok(match response {
            wings_api::servers_server_files_compress::post::Response::Ok(entry) => {
                let archive = support::join(&directory, &entry.name);
                ToolResult::ok(format!("Created {archive}.")).with_display(format!("Created {}", entry.name))
            }
            wings_api::servers_server_files_compress::post::Response::Accepted(_) => {
                ToolResult::ok(format!(
                    "Packing into {archive} in the background; it appears there when done (check with list_files)."
                ))
                .with_display(format!("Packing {name}"))
            }
        })
    }
}

pub struct DecompressFileTool;

#[async_trait::async_trait]
impl Tool for DecompressFileTool {
    fn name(&self) -> &'static str {
        "decompress_file"
    }

    fn description(&self) -> Cow<'static, str> {
        "Extract an archive (zip, tar.gz, …) into the folder it is in. Existing files with the same names are overwritten. Big archives are extracted in the background.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Archive path from the server root."},
            },
            "required": ["path"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Write
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.archive"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        format!("Extract {}", support::shown_path(arguments, "path"))
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let path = file_path(arguments, "path")?;
        let (parent, name) = support::split(&path);
        if server.is_ignored_subtree(&parent) || server.is_ignored(&path, false) {
            return Err(not_found(&path));
        }

        let response = support::wings(context)
            .await?
            .post_servers_server_files_decompress(
                server.uuid,
                &wings_api::servers_server_files_decompress::post::RequestBody {
                    root: parent.as_str().into(),
                    file: name.as_str().into(),
                    foreground: false,
                },
            )
            .await
            .map_err(|err| match err {
                wings_api::client::ApiHttpError::Http(status, _)
                    if status == reqwest::StatusCode::NOT_FOUND =>
                {
                    not_found(&path)
                }
                err => support::wings_error(err),
            })?;
        context.log("file.decompress", json!({"file": path})).await;

        Ok(match response {
            wings_api::servers_server_files_decompress::post::Response::Ok(_) => {
                ToolResult::ok(format!("Extracted {path} into {parent}."))
                    .with_display(format!("Extracted {path}"))
            }
            wings_api::servers_server_files_decompress::post::Response::Accepted(_) => {
                ToolResult::ok(format!(
                    "Extracting {path} into {parent} in the background; the files appear there when done."
                ))
                .with_display(format!("Extracting {path}"))
            }
        })
    }
}

pub struct DeleteFilesTool;

/// The paths to delete: 1 to [`MAX_DELETE`] distinct ones, never the server root.
fn delete_paths(arguments: &Value) -> Result<Vec<String>, ToolError> {
    let mut paths: Vec<String> = Vec::new();
    for path in support::strings(arguments, "paths", MAX_DELETE)? {
        let path = support::path(&Value::String(path), "/")?;
        if path == "/" {
            return Err(ToolError::invalid(
                "The server root itself cannot be deleted.",
            ));
        }
        if !paths.contains(&path) {
            paths.push(path);
        }
    }

    Ok(paths)
}

#[async_trait::async_trait]
impl Tool for DeleteFilesTool {
    fn name(&self) -> &'static str {
        "delete_files"
    }

    fn description(&self) -> Cow<'static, str> {
        "Delete files or folders (up to 20). When the server has a trash they go there and can be restored from Files › Trash for a while; otherwise they are gone for good. Only use it when the user clearly wants these gone.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_DELETE},
            },
            "required": ["paths"],
        })
    }

    fn risk(&self, _arguments: &Value) -> ToolRisk {
        ToolRisk::Destructive
    }

    fn requires(&self, _arguments: &Value) -> Vec<&'static str> {
        vec!["files.delete"]
    }

    fn summarize(&self, arguments: &Value) -> String {
        let paths: Vec<String> = arguments
            .get("paths")
            .and_then(Value::as_array)
            .map(|paths| {
                paths
                    .iter()
                    .map(|path| support::path(path, "/").unwrap_or_else(|_| "?".into()))
                    .collect()
            })
            .unwrap_or_default();
        let more = paths.len().saturating_sub(3);
        let shown = paths.into_iter().take(3).collect::<Vec<_>>().join(", ");

        if more > 0 {
            format!("Delete {shown} and {more} more")
        } else {
            format!("Delete {shown}")
        }
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let mut server = server_copy(context)?;
        let paths = delete_paths(arguments)?;
        if let Some(hidden) = paths.iter().find(|path| server.is_ignored_either(path)) {
            return Err(not_found(hidden));
        }

        // like the delete route: into the trash when the server has one, and what the trash
        // skips deleted for good
        let client = support::wings(context).await?;
        let refused = |err| match err {
            wings_api::client::ApiHttpError::Http(status, _)
                if status == reqwest::StatusCode::NOT_FOUND =>
            {
                ToolError::invalid("One of these paths does not exist; check them with list_files.")
            }
            err => support::wings_error(err),
        };
        let names: Vec<compact_str::CompactString> = paths
            .iter()
            .map(|path| path.trim_start_matches('/').into())
            .collect();

        let mut trashed = Vec::new();
        let mut left = names;
        if crate::minecraft::files::trash_enabled(context.state, &server).await? {
            let skipped = client
                .post_servers_server_files_trash(
                    server.uuid,
                    &wings_api::servers_server_files_trash::post::RequestBody {
                        root: "/".into(),
                        files: left.clone(),
                    },
                )
                .await
                .map_err(refused)?
                .skipped;
            trashed = left
                .iter()
                .filter(|name| !skipped.contains(name))
                .map(|name| format!("/{name}"))
                .collect();
            left = skipped;
        }
        if !left.is_empty()
            && let Err(err) = client
                .post_servers_server_files_delete(
                    server.uuid,
                    &wings_api::servers_server_files_delete::post::RequestBody {
                        root: "/".into(),
                        files: left.clone(),
                    },
                )
                .await
        {
            if trashed.is_empty() {
                return Err(refused(err));
            }

            // the trash already took some: that is done, whatever the delete answered
            context
                .log(
                    "file.delete",
                    json!({"files": paths, "trashed": trashed, "deleted": []}),
                )
                .await;
            let reason = match refused(err) {
                ToolError::Invalid(message) => message,
                ToolError::Failed(err) => {
                    tracing::warn!(server = %server.uuid, "neo could not delete files: {err:#}");
                    "the node did not delete them.".into()
                }
            };

            return Ok(ToolResult::error(format!(
                "{} The rest was not deleted: {reason}",
                delete_report(&trashed, &[])
            )));
        }
        let deleted: Vec<String> = left.iter().map(|name| format!("/{name}")).collect();

        context
            .log(
                "file.delete",
                json!({"files": paths, "trashed": trashed, "deleted": deleted}),
            )
            .await;

        Ok(ToolResult::ok(delete_report(&trashed, &deleted))
            .with_display(format!("Deleted {} item(s)", paths.len())))
    }
}

/// What a delete did, for the model: what went to the trash, what is gone for good.
fn delete_report(trashed: &[String], deleted: &[String]) -> String {
    let mut parts = Vec::new();
    if !trashed.is_empty() {
        parts.push(format!(
            "Moved to the trash (restorable from Files › Trash for a while): {}.",
            trashed.join(", ")
        ));
    }
    if !deleted.is_empty() {
        parts.push(format!(
            "Deleted for good{}: {}.",
            if trashed.is_empty() {
                ""
            } else {
                " (the trash did not take them)"
            },
            deleted.join(", ")
        ));
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_are_escaped() {
        assert_eq!(escape_glob("latest.log"), "latest.log");
        assert_eq!(escape_glob("a*b?[c]{d}\\"), "a\\*b\\?\\[c\\]\\{d\\}\\\\");
    }

    #[test]
    fn numbers_lines() {
        let (lines, total) = numbered("a\nb\nc", 2, 5);
        assert_eq!(lines, vec!["2| b", "3| c"]);
        assert_eq!(total, 3);

        let text = (1..=12)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let (lines, _) = numbered(&text, 9, 2);
        assert_eq!(lines, vec![" 9| 9", "10| 10"]);
        assert!(numbered("a", 5, 3).0.is_empty());
    }

    #[test]
    fn replaces_exactly() {
        assert_eq!(
            replace("a b a", "b", "c", false).unwrap(),
            ("a c a".into(), 1)
        );
        assert_eq!(
            replace("a b a", "a", "x", true).unwrap(),
            ("x b x".into(), 2)
        );
        assert_eq!(replace("a b a", "a", "x", false).unwrap_err(), "2");
        assert_eq!(replace("a", "z", "x", false).unwrap_err(), "not_found");
    }

    #[test]
    fn checks_names_and_paths() {
        assert_eq!(
            archive_names(&json!({"names": ["/world/", " logs ", "../etc", ""]})).unwrap(),
            vec!["world", "logs"]
        );
        assert!(archive_names(&json!({"names": ["../x"]})).is_err());
        assert!(archive_names(&json!({"names": []})).is_err());

        assert_eq!(
            delete_paths(&json!({"paths": ["a.txt", "/a.txt", "logs//old"]})).unwrap(),
            vec!["/a.txt", "/logs/old"]
        );
        assert!(delete_paths(&json!({"paths": ["/"]})).is_err());
        assert!(delete_paths(&json!({"paths": ["a/../../b"]})).is_err());
        let many: Vec<String> = (0..21).map(|n| format!("f{n}")).collect();
        assert!(delete_paths(&json!({ "paths": many })).is_err());

        assert!(file_path(&json!({"path": "/"}), "path").is_err());
        assert_eq!(
            file_path(&json!({"path": "eula.txt"}), "path").unwrap(),
            "/eula.txt"
        );
    }

    #[test]
    fn reports_trash_and_deletes() {
        let a = vec!["/a".to_string()];
        let b = vec!["/b".to_string()];
        assert_eq!(
            delete_report(&a, &[]),
            "Moved to the trash (restorable from Files › Trash for a while): /a."
        );
        assert_eq!(delete_report(&[], &b), "Deleted for good: /b.");
        assert!(
            delete_report(&a, &b).ends_with("Deleted for good (the trash did not take them): /b.")
        );
    }

    #[test]
    fn summaries_use_normalised_paths() {
        assert_eq!(
            DeleteFilesTool.summarize(&json!({"paths": ["a", "b", "c", "d", "e"]})),
            "Delete /a, /b, /c and 2 more"
        );
        assert_eq!(
            WriteFileTool.summarize(&json!({"path": "x.txt", "content": "hello"})),
            "Write /x.txt (5 B)"
        );
        assert_eq!(
            RenameFileTool.summarize(&json!({"from": "a", "to": "b/c"})),
            "Move /a → /b/c"
        );
    }
}
