use super::model::*;
use anyhow::Context;
use futures_util::{StreamExt, TryStreamExt};
use shared::{
    crypt::EncryptedString,
    database::Database,
    models::{
        OrderedJson,
        backup_configuration::BackupConfigs,
        database_host::{DatabaseCredentials, DatabaseType},
    },
};
use sqlx::Row;

const GUARDED_TABLES: [&str; 5] = ["users", "nodes", "servers", "nests", "locations"];
const ENCRYPT_CONCURRENCY: usize = 32;

/// Row counts of the tables an import expects to be empty.
pub async fn target_counts(
    connection: &mut sqlx::PgConnection,
) -> Result<Vec<(&'static str, i64)>, anyhow::Error> {
    let mut counts = Vec::with_capacity(GUARDED_TABLES.len());

    for table in GUARDED_TABLES {
        let count: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
                .fetch_one(&mut *connection)
                .await?;
        counts.push((table, count));
    }

    Ok(counts)
}

pub async fn load_target_snapshot(database: &Database) -> Result<TargetSnapshot, anyhow::Error> {
    let mut snapshot = TargetSnapshot::default();
    let pool = database.write();

    for row in sqlx::query("SELECT uuid, username, email, external_id FROM users")
        .fetch_all(pool)
        .await?
    {
        snapshot.user_uuids.push(row.try_get("uuid")?);
        snapshot.usernames.push(row.try_get("username")?);
        snapshot.emails.push(row.try_get("email")?);
        if let Some(external_id) = row.try_get::<Option<String>, _>("external_id")? {
            snapshot.user_external_ids.push(external_id);
        }
    }

    for row in sqlx::query("SELECT uuid, name, token_id FROM nodes")
        .fetch_all(pool)
        .await?
    {
        snapshot.node_uuids.push(row.try_get("uuid")?);
        snapshot.node_names.push(row.try_get("name")?);
        snapshot
            .node_token_ids
            .push(row.try_get::<String, _>("token_id")?.trim_end().to_string());
    }

    for row in sqlx::query("SELECT uuid, name FROM nests")
        .fetch_all(pool)
        .await?
    {
        snapshot.nest_uuids.push(row.try_get("uuid")?);
        snapshot.nest_names.push(row.try_get("name")?);
    }

    for row in sqlx::query("SELECT uuid, name, source, target FROM mounts")
        .fetch_all(pool)
        .await?
    {
        snapshot.mount_uuids.push(row.try_get("uuid")?);
        snapshot.mount_names.push(row.try_get("name")?);
        snapshot
            .mount_paths
            .push((row.try_get("source")?, row.try_get("target")?));
    }

    for row in sqlx::query("SELECT uuid, uuid_short FROM servers")
        .fetch_all(pool)
        .await?
    {
        snapshot.server_uuids.push(row.try_get("uuid")?);
        snapshot.server_uuid_shorts.push(row.try_get("uuid_short")?);
    }

    snapshot.location_names = sqlx::query_scalar("SELECT name FROM locations")
        .fetch_all(pool)
        .await?;
    snapshot.egg_uuids = sqlx::query_scalar("SELECT uuid FROM nest_eggs")
        .fetch_all(pool)
        .await?;
    snapshot.database_host_names = sqlx::query_scalar("SELECT name FROM database_hosts")
        .fetch_all(pool)
        .await?;
    snapshot.backup_uuids = sqlx::query_scalar("SELECT uuid FROM server_backups")
        .fetch_all(pool)
        .await?;
    snapshot.backup_configuration_names =
        sqlx::query_scalar("SELECT name FROM backup_configurations")
            .fetch_all(pool)
            .await?;

    Ok(snapshot)
}

async fn encrypt_all<T, O>(
    items: &[T],
    encrypt: impl AsyncFn(&T) -> Result<O, anyhow::Error>,
) -> Result<Vec<O>, anyhow::Error> {
    futures_util::stream::iter(items)
        .map(|item| encrypt(item))
        .buffered(ENCRYPT_CONCURRENCY)
        .try_collect()
        .await
}

fn at(table: &str, source_id: i64) -> String {
    format!("failed to write the row imported from {table} #{source_id}")
}

/// Writes the whole plan in one transaction, so a failure leaves the target untouched.
pub async fn write_plan(
    database: &Database,
    plan: &Plan,
    expected_counts: &[(&'static str, i64)],
) -> Result<(), anyhow::Error> {
    tracing::info!("encrypting secrets");

    let mut backup_configs = Vec::with_capacity(plan.backup_configs.len());
    for config in &plan.backup_configs {
        let mut configs = BackupConfigs {
            s3: config.s3.clone(),
            ..Default::default()
        };
        configs.encrypt(database).await?;
        backup_configs.push(serde_json::to_value(configs)?);
    }

    let totp_secrets = encrypt_all(&plan.users, async |user| match &user.totp_secret {
        Some(secret) => Ok(Some(
            EncryptedString::from_plaintext(secret.clone(), database).await?,
        )),
        None => Ok(None),
    })
    .await?;
    let node_tokens = encrypt_all(&plan.nodes, async |node| {
        database.encrypt(node.token.clone()).await
    })
    .await?;
    let host_credentials = encrypt_all(&plan.database_hosts, async |host| {
        let mut credentials = DatabaseCredentials::Details {
            host: host.host.clone(),
            port: host.port,
            username: host.username.clone(),
            password: host.password.clone(),
        };
        credentials.encrypt(database).await?;
        Ok(serde_json::to_value(credentials)?)
    })
    .await?;
    let database_passwords = encrypt_all(&plan.databases, async |db| {
        database.encrypt(db.password.clone()).await
    })
    .await?;

    let mut tx = database.write().begin().await?;

    if target_counts(&mut tx).await? != expected_counts {
        anyhow::bail!(
            "the target database changed while the import was being validated, nothing was written; stop the panel and run the import again"
        );
    }

    for (config, configs) in plan.backup_configs.iter().zip(backup_configs) {
        sqlx::query(
            r#"
            INSERT INTO backup_configurations (uuid, name, description, backup_disk, backup_configs)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(config.uuid)
        .bind(&config.name)
        .bind("automatically generated by import")
        .bind(config.disk)
        .bind(configs)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("failed to write backup configuration {:?}", config.name))?;
    }

    for (user, totp_secret) in plan.users.iter().zip(totp_secrets) {
        sqlx::query(
            r#"
            INSERT INTO users (uuid, external_id, username, email, name_first, name_last, password, admin, totp_enabled, totp_secret, email_verified, language, created)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, true, $11, $12)
            "#,
        )
        .bind(user.uuid)
        .bind(&user.external_id)
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.name_first)
        .bind(&user.name_last)
        .bind(&user.password)
        .bind(user.admin)
        .bind(user.totp_enabled)
        .bind(totp_secret)
        .bind(&user.language)
        .bind(user.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("users", user.source_id))?;
    }
    tracing::info!("wrote {} users", plan.users.len());

    for key in &plan.ssh_keys {
        sqlx::query(
            r#"
            INSERT INTO user_ssh_keys (user_uuid, name, fingerprint, public_key, created)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(key.user_uuid)
        .bind(&key.name)
        .bind(&key.fingerprint)
        .bind(&key.public_key)
        .bind(key.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("user_ssh_keys", key.source_id))?;
    }
    tracing::info!("wrote {} ssh keys", plan.ssh_keys.len());

    for location in &plan.locations {
        sqlx::query(
            r#"
            INSERT INTO locations (uuid, backup_configuration_uuid, name, description, created)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(location.uuid)
        .bind(location.backup_configuration_uuid)
        .bind(&location.name)
        .bind(&location.description)
        .bind(location.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("locations", location.source_id))?;
    }
    tracing::info!("wrote {} locations", plan.locations.len());

    for (node, token) in plan.nodes.iter().zip(node_tokens) {
        sqlx::query(
            r#"
            INSERT INTO nodes (uuid, name, description, deployment_enabled, maintenance_enabled, location_uuid, backup_configuration_uuid, url, public_url, sftp_host, sftp_port, memory, disk, token_id, token, created)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(node.uuid)
        .bind(&node.name)
        .bind(&node.description)
        .bind(node.deployment_enabled)
        .bind(node.maintenance_enabled)
        .bind(node.location_uuid)
        .bind(node.backup_configuration_uuid)
        .bind(&node.url)
        .bind(&node.public_url)
        .bind(&node.sftp_host)
        .bind(node.sftp_port)
        .bind(node.memory)
        .bind(node.disk)
        .bind(&node.token_id)
        .bind(token)
        .bind(node.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("nodes", node.source_id))?;
    }
    tracing::info!("wrote {} nodes", plan.nodes.len());

    for nest in &plan.nests {
        sqlx::query(
            r#"
            INSERT INTO nests (uuid, author, name, description, created)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(nest.uuid)
        .bind(&nest.author)
        .bind(&nest.name)
        .bind(&nest.description)
        .bind(nest.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("nests", nest.source_id))?;
    }
    tracing::info!("wrote {} nests", plan.nests.len());

    for egg in &plan.eggs {
        sqlx::query(
            r#"
            INSERT INTO nest_eggs (
                uuid, nest_uuid, author, name, description, features, docker_images,
                file_denylist, config_files, config_startup, config_stop,
                config_script, startup_commands, force_outgoing_ip, created
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(egg.uuid)
        .bind(egg.nest_uuid)
        .bind(&egg.author)
        .bind(&egg.name)
        .bind(&egg.description)
        .bind(&egg.features)
        .bind(OrderedJson(&egg.docker_images))
        .bind(&egg.file_denylist)
        .bind(serde_json::to_value(&egg.config_files)?)
        .bind(serde_json::to_value(&egg.config_startup)?)
        .bind(serde_json::to_value(&egg.config_stop)?)
        .bind(serde_json::to_value(&egg.config_script)?)
        .bind(OrderedJson(&egg.startup_commands))
        .bind(egg.force_outgoing_ip)
        .bind(egg.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("eggs", egg.source_id))?;
    }
    tracing::info!("wrote {} eggs", plan.eggs.len());

    for variable in &plan.egg_variables {
        sqlx::query(
            r#"
            INSERT INTO nest_egg_variables (uuid, egg_uuid, name, description, order_, env_variable, default_value, user_viewable, user_editable, rules, created)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(variable.uuid)
        .bind(variable.egg_uuid)
        .bind(&variable.name)
        .bind(&variable.description)
        .bind(variable.order)
        .bind(&variable.env_variable)
        .bind(&variable.default_value)
        .bind(variable.user_viewable)
        .bind(variable.user_editable)
        .bind(&variable.rules)
        .bind(variable.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("egg_variables", variable.source_id))?;
    }
    tracing::info!("wrote {} egg variables", plan.egg_variables.len());

    for (host, credentials) in plan.database_hosts.iter().zip(host_credentials) {
        sqlx::query(
            r#"
            INSERT INTO database_hosts (uuid, name, type, deployment_enabled, credentials, created)
            VALUES ($1, $2, $3, true, $4, $5)
            "#,
        )
        .bind(host.uuid)
        .bind(&host.name)
        .bind(DatabaseType::Mysql)
        .bind(credentials)
        .bind(host.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("database_hosts", host.source_id))?;

        for node_uuid in &host.node_uuids {
            sqlx::query(
                "INSERT INTO node_database_hosts (node_uuid, database_host_uuid) VALUES ($1, $2)",
            )
            .bind(node_uuid)
            .bind(host.uuid)
            .execute(&mut *tx)
            .await
            .with_context(|| at("database_hosts", host.source_id))?;
        }

        for location_uuid in &host.location_uuids {
            sqlx::query(
                "INSERT INTO location_database_hosts (location_uuid, database_host_uuid) VALUES ($1, $2)",
            )
            .bind(location_uuid)
            .bind(host.uuid)
            .execute(&mut *tx)
            .await
            .with_context(|| at("database_hosts", host.source_id))?;
        }
    }
    tracing::info!("wrote {} database hosts", plan.database_hosts.len());

    for server in &plan.servers {
        sqlx::query(
            r#"
            INSERT INTO servers (
                uuid, uuid_short, external_id, node_uuid, name, description, status, suspended,
                owner_uuid, memory, swap, disk, io_weight, cpu, pinned_cpus, allocation_limit,
                database_limit, backup_limit, schedule_limit, egg_uuid, startup, image, labels, created
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
            "#,
        )
        .bind(server.uuid)
        .bind(server.uuid_short)
        .bind(&server.external_id)
        .bind(server.node_uuid)
        .bind(&server.name)
        .bind(&server.description)
        .bind(server.status)
        .bind(server.suspended)
        .bind(server.owner_uuid)
        .bind(server.memory)
        .bind(server.swap)
        .bind(server.disk)
        .bind(server.io_weight)
        .bind(server.cpu)
        .bind(&server.pinned_cpus)
        .bind(server.allocation_limit)
        .bind(server.database_limit)
        .bind(server.backup_limit)
        .bind(server.schedule_limit)
        .bind(server.egg_uuid)
        .bind(&server.startup)
        .bind(&server.image)
        .bind(OrderedJson(&server.labels))
        .bind(server.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("servers", server.source_id))?;
    }
    tracing::info!("wrote {} servers", plan.servers.len());

    for (db, password) in plan.databases.iter().zip(database_passwords) {
        sqlx::query(
            r#"
            INSERT INTO server_databases (server_uuid, database_host_uuid, name, username, password, created)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(db.server_uuid)
        .bind(db.database_host_uuid)
        .bind(&db.name)
        .bind(&db.username)
        .bind(password)
        .bind(db.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("databases", db.source_id))?;
    }
    tracing::info!("wrote {} server databases", plan.databases.len());

    for variable in &plan.server_variables {
        sqlx::query(
            r#"
            INSERT INTO server_variables (server_uuid, variable_uuid, value, created)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(variable.server_uuid)
        .bind(variable.variable_uuid)
        .bind(&variable.value)
        .bind(variable.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("server_variables", variable.source_id))?;
    }
    tracing::info!("wrote {} server variables", plan.server_variables.len());

    for backup in &plan.backups {
        let streaming = matches!(
            backup.disk,
            shared::models::server_backup::BackupDisk::DdupBak
                | shared::models::server_backup::BackupDisk::Btrfs
                | shared::models::server_backup::BackupDisk::Zfs
                | shared::models::server_backup::BackupDisk::Restic
        );

        sqlx::query(
            r#"
            INSERT INTO server_backups (uuid, server_uuid, node_uuid, backup_configuration_uuid, name, successful, browsable, streaming, locked, ignored_files, disk, checksum, bytes, upload_id, completed, deleted, created)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            "#,
        )
        .bind(backup.uuid)
        .bind(backup.server_uuid)
        .bind(backup.node_uuid)
        .bind(backup.backup_configuration_uuid)
        .bind(&backup.name)
        .bind(backup.successful)
        .bind(streaming)
        .bind(streaming)
        .bind(backup.locked)
        .bind(&backup.ignored_files)
        .bind(backup.disk)
        .bind(&backup.checksum)
        .bind(backup.bytes)
        .bind(&backup.upload_id)
        .bind(backup.completed.map(|completed| completed.naive_utc()))
        .bind(backup.deleted.map(|deleted| deleted.naive_utc()))
        .bind(backup.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("backups", backup.source_id))?;
    }
    tracing::info!("wrote {} backups", plan.backups.len());

    for subuser in &plan.subusers {
        sqlx::query(
            r#"
            INSERT INTO server_subusers (server_uuid, user_uuid, permissions, ignored_files, created)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(subuser.server_uuid)
        .bind(subuser.user_uuid)
        .bind(&subuser.permissions)
        .bind(&[] as &[&str])
        .bind(subuser.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("subusers", subuser.source_id))?;
    }
    tracing::info!("wrote {} subusers", plan.subusers.len());

    for mount in &plan.mounts {
        sqlx::query(
            r#"
            INSERT INTO mounts (uuid, name, description, source, target, read_only, user_mountable)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(mount.uuid)
        .bind(&mount.name)
        .bind(&mount.description)
        .bind(&mount.source)
        .bind(&mount.target)
        .bind(mount.read_only)
        .bind(mount.user_mountable)
        .execute(&mut *tx)
        .await
        .with_context(|| at("mounts", mount.source_id))?;
    }

    for (table, column, links) in [
        ("nest_egg_mounts", "egg_uuid", &plan.egg_mounts),
        ("node_mounts", "node_uuid", &plan.node_mounts),
        ("server_mounts", "server_uuid", &plan.server_mounts),
    ] {
        for (target_uuid, mount_uuid) in links {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "INSERT INTO {table} ({column}, mount_uuid) VALUES ($1, $2)"
            )))
            .bind(target_uuid)
            .bind(mount_uuid)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("failed to write a mount assignment into {table}"))?;
        }
    }
    tracing::info!("wrote {} mounts", plan.mounts.len());

    for schedule in &plan.schedules {
        sqlx::query(
            r#"
            INSERT INTO server_schedules (uuid, server_uuid, name, enabled, triggers, condition, last_run, created)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(schedule.uuid)
        .bind(schedule.server_uuid)
        .bind(&schedule.name)
        .bind(schedule.enabled)
        .bind(serde_json::to_value(&schedule.triggers)?)
        .bind(serde_json::to_value(&schedule.condition)?)
        .bind(schedule.last_run.map(|last_run| last_run.naive_utc()))
        .bind(schedule.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("schedules", schedule.source_id))?;
    }
    tracing::info!("wrote {} schedules", plan.schedules.len());

    for step in &plan.schedule_steps {
        sqlx::query(
            r#"
            INSERT INTO server_schedule_steps (schedule_uuid, action, order_, created)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(step.schedule_uuid)
        .bind(serde_json::to_value(&step.action)?)
        .bind(step.order)
        .bind(step.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("tasks", step.source_id))?;
    }
    tracing::info!("wrote {} schedule steps", plan.schedule_steps.len());

    for allocation in &plan.allocations {
        sqlx::query(
            r#"
            INSERT INTO node_allocations (uuid, node_uuid, ip, ip_alias, port, created)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(allocation.uuid)
        .bind(allocation.node_uuid)
        .bind(allocation.ip)
        .bind(&allocation.ip_alias)
        .bind(allocation.port)
        .bind(allocation.created.naive_utc())
        .execute(&mut *tx)
        .await
        .with_context(|| at("allocations", allocation.source_id))?;
    }
    tracing::info!("wrote {} allocations", plan.allocations.len());

    for allocation in &plan.server_allocations {
        sqlx::query(
            r#"
            INSERT INTO server_allocations (uuid, server_uuid, allocation_uuid, notes, created, locked)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#, // plugged.host: locked
        )
        .bind(allocation.uuid)
        .bind(allocation.server_uuid)
        .bind(allocation.allocation_uuid)
        .bind(&allocation.notes)
        .bind(allocation.created.naive_utc())
        .bind(allocation.locked) // plugged.host
        .execute(&mut *tx)
        .await
        .with_context(|| at("allocations", allocation.source_id))?;

        if allocation.primary {
            sqlx::query("UPDATE servers SET allocation_uuid = $1 WHERE servers.uuid = $2")
                .bind(allocation.uuid)
                .bind(allocation.server_uuid)
                .execute(&mut *tx)
                .await
                .with_context(|| at("allocations", allocation.source_id))?;
        }
    }

    super::extras::write(&mut tx, plan).await?; // plugged.host: Pelican's own data (extras.rs)
    super::limbo::write(&mut tx, plan).await?; // plugged.host: limbo's servers (limbo.rs)

    tx.commit().await?;

    Ok(())
}
