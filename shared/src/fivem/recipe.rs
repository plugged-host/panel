//! plugged.host: runs a txAdmin recipe (engine 3) into a deploy folder through Wings, the way
//! txAdmin's own deployer would, without txAdmin (docs/features/fivem.md#frameworks).
//!
//! The task set is txAdmin's: `download_github`, `download_file`, `unzip`, `move_path`,
//! `copy_path`, `remove_path`, `ensure_dir`, `write_file`, `replace_string`, `connect_database`,
//! `query_database`, `load_vars` and `waste_time`. Every path a recipe names is relative to the
//! deploy folder and kept inside it. Downloads only come from the URLs the recipe names, and the
//! recipe only comes from txAdmin's index, so this runs what txAdmin's "Popular recipes" would.
//!
//! The data never passes through the panel: Wings downloads (its pull takes responses without a
//! `Content-Length`, so GitHub's archives need no relay), extracts, renames and copies; the panel
//! only sees directory listings and the small text files a recipe edits.

use crate::{
    minecraft::{content, files},
    models::server::Server,
};
use std::collections::{BTreeMap, HashMap};

/// Where a task's downloads and extractions are staged, inside the deploy folder.
pub const SCRATCH: &str = ".install-tmp";
const READ_LIMIT: u64 = 16 * 1024 * 1024;
const SQL_LIMIT: u64 = 32 * 1024 * 1024;
const VARS_LIMIT: u64 = 1024 * 1024;

pub type Task = serde_json::Value;

fn text<'a>(task: &'a Task, key: &str) -> Option<&'a str> {
    task.get(key).and_then(serde_json::Value::as_str)
}

/// txAdmin's booleans come as YAML booleans, strings or numbers.
pub fn flag(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::String(value)) => matches!(value.as_str(), "true" | "1"),
        Some(serde_json::Value::Number(value)) => value.as_i64() == Some(1),
        _ => false,
    }
}

/// A task's own timeout, 60 seconds to 30 minutes (15 when it names none).
pub fn timeout(task: &Task) -> std::time::Duration {
    let seconds = match task.get("timeoutSeconds") {
        Some(serde_json::Value::Number(value)) => value.as_u64(),
        Some(serde_json::Value::String(value)) => value.trim().parse().ok(),
        _ => None,
    }
    .unwrap_or(900);

    std::time::Duration::from_secs(seconds.clamp(60, 1800))
}

/// Fills a text's `{{variables}}`, leaving unknown ones as they are.
pub fn fill(text: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end)
                if !after[..end].is_empty()
                    && after[..end]
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_') =>
            {
                match vars.get(&after[..end]) {
                    Some(value) => out.push_str(value),
                    None => out.push_str(&rest[start..start + 4 + end]),
                }
                rest = &after[end + 2..];
            }
            _ => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);

    out
}

/// A recipe path as a server-root path inside the deploy folder.
pub fn resolve(deploy: &str, path: Option<&str>) -> Result<String, anyhow::Error> {
    let path = path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| anyhow::anyhow!("a task is missing its path"))?;
    let resolved = super::txadmin::normalize(&format!("{deploy}/{path}"));

    if resolved != deploy && !resolved.starts_with(&format!("{deploy}/")) {
        anyhow::bail!("\"{path}\" is outside the server folder");
    }

    Ok(resolved)
}

/// The archive URL of a GitHub repository at a ref (the default branch when there is none).
/// GitHub's archive URL needs no API call, so there is no API rate limit either.
pub fn github_archive(source: &str, reference: Option<&str>) -> Result<String, anyhow::Error> {
    let trimmed = source.trim();
    let path = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let valid = |part: &str, extra: &[u8]| {
        !part.is_empty()
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || extra.contains(&b))
    };
    let (owner, repository) = path
        .split_once('/')
        .filter(|(owner, repository)| valid(owner, b"") && valid(repository, b"._"))
        .ok_or_else(|| anyhow::anyhow!("\"{trimmed}\" is not a GitHub repository"))?;

    let reference = reference.map(str::trim).unwrap_or("HEAD");
    if reference.is_empty()
        || reference.contains("..")
        || !reference
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b'-'))
    {
        anyhow::bail!("\"{reference}\" is not a git ref");
    }

    Ok(format!(
        "https://github.com/{owner}/{repository}/archive/{reference}.zip"
    ))
}

/// A task in a few words, for the progress line.
pub fn describe(task: &Task) -> String {
    let action = text(task, "action").unwrap_or("step");
    let base = |value: Option<&str>| {
        value
            .unwrap_or_default()
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string()
    };
    let target = match action {
        "download_github" => text(task, "src")
            .unwrap_or_default()
            .trim_start_matches("https://github.com/")
            .trim_end_matches('/')
            .to_string(),
        "download_file" => {
            base(text(task, "url").map(|url| url.split(['?', '#']).next().unwrap_or_default()))
        }
        "unzip" | "move_path" | "copy_path" => base(text(task, "src")),
        "remove_path" | "ensure_dir" => text(task, "path").unwrap_or_default().to_string(),
        "write_file" | "replace_string" => match task.get("file") {
            Some(serde_json::Value::Array(files)) => files
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            _ => text(task, "file").unwrap_or_default().to_string(),
        },
        "query_database" => base(text(task, "file").or(Some("query"))),
        _ => String::new(),
    };
    let description = format!("{} {target}", action.replace('_', " "));

    description.trim().chars().take(120).collect()
}

/// Whether a recipe has tasks that need a database.
pub fn needs_database(recipe: &serde_json::Value) -> bool {
    recipe
        .get("tasks")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .any(|task| {
            matches!(
                text(task, "action"),
                Some("connect_database" | "query_database")
            )
        })
}

fn parent(path: &str) -> &str {
    path.trim_matches('/')
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent)
}

fn name(path: &str) -> &str {
    path.trim_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
}

fn wings(err: wings_api::client::ApiHttpError) -> anyhow::Error {
    match err {
        wings_api::client::ApiHttpError::Http(status, error) => {
            anyhow::anyhow!("the node answered {status}: {}", error.error)
        }
        err => anyhow::anyhow!("{err:?}"),
    }
}

/// The file operations a recipe needs, built from Wings' file API. A recipe thinks in node's
/// fs-extra ("extract this zip into that folder", "move this folder there, merging"); those are
/// [`Files::extract_into`] and [`Files::merge_move`]. Paths are server-root paths without a
/// leading slash, already resolved by the caller.
pub struct Files<'a> {
    state: &'a crate::State,
    server: &'a Server,
    client: wings_api::client::WingsClient,
    /// directory → name → is a directory
    listings: HashMap<String, BTreeMap<String, bool>>,
}

impl<'a> Files<'a> {
    pub async fn new(state: &'a crate::State, server: &'a Server) -> Result<Self, anyhow::Error> {
        Ok(Self {
            state,
            server,
            client: files::client(state, server).await?,
            listings: HashMap::new(),
        })
    }

    pub fn client(&self) -> &wings_api::client::WingsClient {
        &self.client
    }

    /// name → is a directory; empty when the directory is not there.
    pub async fn list(&mut self, directory: &str) -> Result<BTreeMap<String, bool>, anyhow::Error> {
        let directory = directory.trim_matches('/').to_string();
        if let Some(listing) = self.listings.get(&directory) {
            return Ok(listing.clone());
        }

        let entries = match self
            .client
            .get_servers_server_files_list_directory(
                self.server.uuid,
                &wings_api::servers_server_files_list_directory::get::Query {
                    directory: Some(format!("/{directory}").into()),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(entries) => entries,
            Err(wings_api::client::ApiHttpError::Http(status, _))
                if status == reqwest::StatusCode::NOT_FOUND =>
            {
                Vec::new()
            }
            Err(err) => return Err(wings(err)),
        };
        let listing: BTreeMap<String, bool> = entries
            .into_iter()
            .map(|entry| (entry.name.to_string(), entry.directory && !entry.file))
            .collect();
        self.listings.insert(directory, listing.clone());

        Ok(listing)
    }

    pub async fn exists(&mut self, path: &str) -> Result<bool, anyhow::Error> {
        Ok(self.list(parent(path)).await?.contains_key(name(path)))
    }

    pub async fn is_dir(&mut self, path: &str) -> Result<bool, anyhow::Error> {
        Ok(self
            .list(parent(path))
            .await?
            .get(name(path))
            .copied()
            .unwrap_or(false))
    }

    /// Drops what is known about a path: its parent's listing, and everything under it.
    fn forget(&mut self, path: &str) {
        let path = path.trim_matches('/');
        let parent = parent(path).to_string();
        self.listings.retain(|cached, _| {
            *cached != parent
                && !(path.is_empty() || cached == path || cached.starts_with(&format!("{path}/")))
        });
    }

    pub async fn make_dir(&mut self, path: &str) -> Result<(), anyhow::Error> {
        let path = path.trim_matches('/');
        if path.is_empty() || self.is_dir(path).await? {
            return Ok(());
        }

        self.client
            .post_servers_server_files_create_directory(
                self.server.uuid,
                &wings_api::servers_server_files_create_directory::post::RequestBody {
                    root: "/".into(),
                    name: path.into(),
                },
            )
            .await
            .map_err(wings)?;
        self.forget(path);

        Ok(())
    }

    /// Deletes for good: everything this removes is the install's own.
    pub async fn delete(&mut self, path: &str) -> Result<(), anyhow::Error> {
        if !self.exists(path).await? {
            return Ok(());
        }

        self.client
            .post_servers_server_files_delete(
                self.server.uuid,
                &wings_api::servers_server_files_delete::post::RequestBody {
                    root: format!("/{}", parent(path)).into(),
                    files: vec![name(path).into()],
                },
            )
            .await
            .map_err(wings)?;
        self.forget(path);

        Ok(())
    }

    pub async fn rename(&mut self, from: &str, to: &str) -> Result<(), anyhow::Error> {
        let renamed = self
            .client
            .put_servers_server_files_rename_with(
                self.server.uuid,
                &wings_api::servers_server_files_rename::put::RequestBody {
                    root: "/".into(),
                    files: vec![
                        wings_api::servers_server_files_rename::put::RequestBodyFiles {
                            from: from.into(),
                            to: to.into(),
                        },
                    ],
                },
                &wings_api::servers_server_files_rename::put::Extra {
                    create_directories: true,
                    ..Default::default()
                },
            )
            .await
            .map_err(wings)?
            .renamed;
        self.forget(from);
        self.forget(to);
        if renamed != 1 {
            anyhow::bail!("{} could not be moved to {to}", name(from));
        }

        Ok(())
    }

    pub async fn read(&mut self, path: &str, limit: u64) -> Result<String, anyhow::Error> {
        files::read_text(&self.client, self.server, path, limit)
            .await?
            .ok_or_else(|| anyhow::anyhow!("{path} is not there"))
    }

    pub async fn write(&mut self, path: &str, contents: String) -> Result<(), anyhow::Error> {
        self.make_dir(parent(path)).await?;
        files::write_text(&self.client, self.server, path, contents).await?;
        self.forget(path);

        Ok(())
    }

    /// Wings downloads a URL into a file and answers when it is done.
    pub async fn pull(
        &mut self,
        url: &str,
        path: &str,
        timeout: std::time::Duration,
    ) -> Result<(), anyhow::Error> {
        self.delete(path).await?;
        let pulled = tokio::time::timeout(
            timeout,
            content::pull_now(self.state, self.server, url, name(path), parent(path)),
        )
        .await;
        self.forget(path);

        match pulled {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => anyhow::bail!("downloading {url} failed: {}", err.message()),
            Err(_) => anyhow::bail!("downloading {url} took too long"),
        }
    }

    /// Extracts an archive next to itself.
    pub async fn decompress(&mut self, archive: &str) -> Result<(), anyhow::Error> {
        let result = self
            .client
            .post_servers_server_files_decompress(
                self.server.uuid,
                &wings_api::servers_server_files_decompress::post::RequestBody {
                    root: format!("/{}", parent(archive)).into(),
                    file: name(archive).into(),
                    foreground: true,
                },
            )
            .await;
        self.forget(parent(archive));
        result
            .map(|_| ())
            .map_err(|err| anyhow::anyhow!("extracting {} failed: {}", name(archive), wings(err)))
    }

    /// Extracts an archive into a folder, merging with what it holds (txAdmin's `unzip`). Wings
    /// extracts next to the archive, so it goes into a scratch folder, is extracted there and put
    /// back, and what came out is merged into the destination.
    pub async fn extract_into(
        &mut self,
        archive: &str,
        destination: &str,
        scratch: &str,
    ) -> Result<(), anyhow::Error> {
        self.make_dir(scratch).await?;
        let staged = format!("{scratch}/{}", name(archive));
        self.rename(archive, &staged).await?;
        let extracted = self.decompress(&staged).await;
        self.rename(&staged, archive).await?;
        extracted?;

        for entry in self.list(scratch).await?.into_keys() {
            Box::pin(self.merge_move(
                &format!("{scratch}/{entry}"),
                &format!("{destination}/{entry}"),
            ))
            .await?;
        }

        self.delete(scratch).await
    }

    /// Moves a file or folder, merging a folder into one that is already there and replacing
    /// files in the way (fs-extra's move, without copying).
    pub async fn merge_move(&mut self, from: &str, to: &str) -> Result<(), anyhow::Error> {
        if !self.exists(to).await? {
            return self.rename(from, to).await;
        }
        if !self.is_dir(from).await? || !self.is_dir(to).await? {
            self.delete(to).await?;
            return self.rename(from, to).await;
        }

        for entry in self.list(from).await?.into_keys() {
            Box::pin(self.merge_move(&format!("{from}/{entry}"), &format!("{to}/{entry}"))).await?;
        }

        self.delete(from).await
    }

    /// Copies a file or folder (txAdmin's `copy_path`), file by file so a folder merges.
    pub async fn copy(
        &mut self,
        from: &str,
        to: &str,
        overwrite: bool,
    ) -> Result<(), anyhow::Error> {
        if self.is_dir(from).await? {
            self.make_dir(to).await?;
            for entry in self.list(from).await?.into_keys() {
                Box::pin(self.copy(
                    &format!("{from}/{entry}"),
                    &format!("{to}/{entry}"),
                    overwrite,
                ))
                .await?;
            }
            return Ok(());
        }
        if !self.exists(from).await? {
            anyhow::bail!("{from} is not there");
        }
        if self.exists(to).await? {
            if !overwrite {
                return Ok(());
            }
            self.delete(to).await?;
        }

        self.make_dir(parent(to)).await?;
        self.client
            .post_servers_server_files_copy_many(
                self.server.uuid,
                &wings_api::servers_server_files_copy_many::post::RequestBody {
                    root: "/".into(),
                    files: vec![
                        wings_api::servers_server_files_copy_many::post::RequestBodyFiles {
                            from: from.into(),
                            to: to.into(),
                        },
                    ],
                    overwrite: true,
                    foreground: true,
                },
            )
            .await
            .map_err(wings)?;
        self.forget(to);

        Ok(())
    }
}

/// How the recipe's database tasks log in: as the database's own user, from the panel.
#[derive(Clone)]
pub struct DatabaseLogin {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub name: String,
}

impl DatabaseLogin {
    async fn connect(&self) -> Result<sqlx::MySqlConnection, anyhow::Error> {
        use sqlx::Connection;

        let options = sqlx::mysql::MySqlConnectOptions::new()
            .host(crate::net::unbracket(&self.host))
            .port(self.port)
            .username(&self.user)
            .password(&self.password)
            .database(&self.name)
            .charset("utf8mb4");

        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            sqlx::MySqlConnection::connect_with(&options),
        )
        .await
        .map_err(|_| anyhow::anyhow!("the database did not answer"))?
        .map_err(|err| anyhow::anyhow!("connecting to the database failed: {err}"))
    }
}

pub struct Runner<'a> {
    pub files: Files<'a>,
    deploy: String,
    pub vars: BTreeMap<String, String>,
    database: Option<DatabaseLogin>,
}

impl<'a> Runner<'a> {
    pub fn new(
        files: Files<'a>,
        deploy: String,
        vars: BTreeMap<String, String>,
        database: Option<DatabaseLogin>,
    ) -> Self {
        Self {
            files,
            deploy,
            vars,
            database,
        }
    }

    fn path(&self, path: Option<&str>) -> Result<String, anyhow::Error> {
        resolve(&self.deploy, path)
    }

    fn scratch(&self, index: usize) -> String {
        format!("{}/{SCRATCH}/{index}", self.deploy)
    }

    /// Runs one task; the error names the step.
    pub async fn run(&mut self, task: &Task, index: usize) -> Result<(), anyhow::Error> {
        self.task(task, index)
            .await
            .map_err(|err| anyhow::anyhow!("step {} ({}): {err:#}", index + 1, describe(task)))
    }

    async fn task(&mut self, task: &Task, index: usize) -> Result<(), anyhow::Error> {
        match text(task, "action").unwrap_or_default() {
            "download_github" => self.download_github(task, index).await,
            "download_file" => {
                let url = text(task, "url").unwrap_or_default();
                if !url.starts_with("https://") || url.chars().any(char::is_whitespace) {
                    anyhow::bail!("\"{url}\" is not an https URL");
                }
                let path = self.path(text(task, "path"))?;
                self.files.pull(url, &path, timeout(task)).await
            }
            "unzip" => {
                let (archive, destination) = (
                    self.path(text(task, "src"))?,
                    self.path(text(task, "dest"))?,
                );
                let scratch = self.scratch(index);
                self.files
                    .extract_into(&archive, &destination, &scratch)
                    .await
            }
            "move_path" => {
                let (from, to) = (
                    self.path(text(task, "src"))?,
                    self.path(text(task, "dest"))?,
                );
                if flag(task.get("overwrite")) {
                    self.files.delete(&to).await?;
                }
                self.files.merge_move(&from, &to).await
            }
            "copy_path" => {
                let (from, to) = (
                    self.path(text(task, "src"))?,
                    self.path(text(task, "dest"))?,
                );
                self.files
                    .copy(&from, &to, flag(task.get("overwrite")))
                    .await
            }
            "remove_path" => {
                let path = self.path(text(task, "path"))?;
                self.files.delete(&path).await
            }
            "ensure_dir" => {
                let path = self.path(text(task, "path"))?;
                self.files.make_dir(&path).await
            }
            "write_file" => {
                let path = self.path(text(task, "file"))?;
                let mut data = text(task, "data").unwrap_or_default().to_string();
                if flag(task.get("append")) && self.files.exists(&path).await? {
                    data = self.files.read(&path, READ_LIMIT).await? + &data;
                }
                self.files.write(&path, data).await
            }
            "replace_string" => self.replace_string(task).await,
            "load_vars" => {
                let path = self.path(text(task, "src"))?;
                let raw = self.files.read(&path, VARS_LIMIT).await?;
                let vars: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_str(&raw)
                        .map_err(|_| anyhow::anyhow!("the variables file is not a JSON object"))?;
                for (name, value) in vars {
                    let value = match value {
                        serde_json::Value::String(value) => value,
                        serde_json::Value::Number(value) => value.to_string(),
                        serde_json::Value::Bool(value) => value.to_string(),
                        _ => continue,
                    };
                    if !name.is_empty()
                        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    {
                        self.vars.insert(name, value);
                    }
                }
                Ok(())
            }
            "connect_database" => {
                let login = self
                    .database
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("this recipe needs a database"))?;
                use sqlx::Connection;
                login.connect().await?.close().await.ok();
                Ok(())
            }
            "query_database" => self.query(task).await,
            "waste_time" => Ok(()),
            action => {
                anyhow::bail!("this recipe uses \"{action}\", which the installer does not support")
            }
        }
    }

    async fn download_github(&mut self, task: &Task, index: usize) -> Result<(), anyhow::Error> {
        let url = github_archive(text(task, "src").unwrap_or_default(), text(task, "ref"))?;
        let destination = self.path(text(task, "dest"))?;
        let scratch = self.scratch(index);
        let archive = format!("{scratch}/repository.zip");

        self.files.make_dir(&scratch).await?;
        self.files.pull(&url, &archive, timeout(task)).await?;
        self.files.decompress(&archive).await?;
        self.files.delete(&archive).await?;

        // GitHub archives hold one folder, <repository>-<ref>
        let root = self
            .files
            .list(&scratch)
            .await?
            .into_iter()
            .find(|(_, directory)| *directory)
            .map(|(name, _)| name)
            .ok_or_else(|| anyhow::anyhow!("the archive was empty"))?;
        let subpath = super::txadmin::normalize(text(task, "subpath").unwrap_or_default());
        let from = if subpath.is_empty() {
            format!("{scratch}/{root}")
        } else {
            format!("{scratch}/{root}/{subpath}")
        };
        if !self.files.exists(&from).await? {
            anyhow::bail!("the repository has no \"{subpath}\"");
        }

        self.files.merge_move(&from, &destination).await?;
        self.files.delete(&scratch).await
    }

    /// txAdmin's three modes: `template` (the default) fills the replacement's variables,
    /// `all_vars` fills every `{{variable}}` in the file, `literal` does neither.
    async fn replace_string(&mut self, task: &Task) -> Result<(), anyhow::Error> {
        let paths: Vec<&str> = match task.get("file") {
            Some(serde_json::Value::Array(files)) => {
                files.iter().filter_map(serde_json::Value::as_str).collect()
            }
            Some(serde_json::Value::String(file)) => vec![file.as_str()],
            _ => Vec::new(),
        };
        let search = text(task, "search").unwrap_or_default();
        let replace = text(task, "replace").unwrap_or_default();
        let mode = text(task, "mode").unwrap_or("template");

        for path in paths {
            let path = self.path(Some(path))?;
            let original = self.files.read(&path, READ_LIMIT).await?;
            let changed = match mode {
                "all_vars" => fill(&original, &self.vars),
                _ if search.is_empty() => original.clone(),
                "literal" => original.replace(search, replace),
                _ => original.replace(search, &fill(replace, &self.vars)),
            };
            if changed != original {
                self.files.write(&path, changed).await?;
            }
        }

        Ok(())
    }

    async fn query(&mut self, task: &Task) -> Result<(), anyhow::Error> {
        let sql = match text(task, "file") {
            Some(file) => {
                let path = self.path(Some(file))?;
                self.files.read(&path, SQL_LIMIT).await?
            }
            None => text(task, "query").unwrap_or_default().to_string(),
        };
        if sql.trim().is_empty() {
            return Ok(());
        }
        let login = self
            .database
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("this recipe needs a database"))?;

        let mut connection = login.connect().await?;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql)).execute(&mut connection),
        )
        .await;
        use sqlx::Connection;
        connection.close().await.ok();

        match result {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(err)) => anyhow::bail!("the database refused the query: {err}"),
            Err(_) => anyhow::bail!("the query took too long"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_fill_and_unknown_stay() {
        let vars = BTreeMap::from([("serverName".to_string(), "My {{x}} Server".to_string())]);
        assert_eq!(
            fill(
                "sv_hostname \"{{serverName}}\" {{missing}} {{ bad }} {{",
                &vars
            ),
            "sv_hostname \"My {{x}} Server\" {{missing}} {{ bad }} {{"
        );
    }

    #[test]
    fn paths_stay_in_the_deploy_folder() {
        let deploy = "txData/ESX_ABC123.base";
        assert_eq!(
            resolve(deploy, Some("./tmp/../resources")).unwrap(),
            "txData/ESX_ABC123.base/resources"
        );
        assert_eq!(resolve(deploy, Some("./")).unwrap(), deploy);
        assert!(resolve(deploy, Some("../../etc")).is_err());
        assert!(resolve(deploy, Some("../ESX_ABC123.base2/x")).is_err());
        assert!(resolve(deploy, None).is_err());
    }

    #[test]
    fn github_sources_and_refs() {
        assert_eq!(
            github_archive("https://github.com/esx-framework/esx_core", Some("v1.10")).unwrap(),
            "https://github.com/esx-framework/esx_core/archive/v1.10.zip"
        );
        assert_eq!(
            github_archive("overextended/ox_lib.git/", None).unwrap(),
            "https://github.com/overextended/ox_lib/archive/HEAD.zip"
        );
        assert!(github_archive("https://example.com/a/b", None).is_err());
        assert!(github_archive("a/b", Some("../main")).is_err());
        assert!(github_archive("a/b/c", None).is_err());
    }

    #[test]
    fn tasks_described_and_flags_read() {
        let task = serde_json::json!({"action": "download_file", "url": "https://x.example/a/b.zip?x=1", "timeoutSeconds": "5"});
        assert_eq!(describe(&task), "download file b.zip");
        assert_eq!(timeout(&task).as_secs(), 60);
        assert_eq!(timeout(&serde_json::json!({})).as_secs(), 900);
        assert!(flag(Some(&serde_json::json!("true"))) && !flag(Some(&serde_json::json!(false))));
        assert!(needs_database(
            &serde_json::json!({"tasks": [{"action": "query_database"}]})
        ));
        assert!(!needs_database(
            &serde_json::json!({"tasks": [{"action": "unzip"}]})
        ));
        assert_eq!(name("a/b/c"), "c");
        assert_eq!(parent("a/b/c"), "a/b");
        assert_eq!(parent("c"), "");
    }
}
