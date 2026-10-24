//! plugged.host: moving a server's row to another node without Wings' transfer, for when the old
//! node cannot take part: node failover (docs/features/failover.md), and limbo later. The files
//! come back by restoring a backup on the new node; the old node's copy is removed separately.

use super::Server;
use crate::models::ByUuid;
use std::collections::HashMap;

pub struct RehomeOptions {
    pub node_uuid: uuid::Uuid,
    /// The primary allocation on the new node (`None` for a server without one).
    pub allocation_uuid: Option<uuid::Uuid>,
    pub allocation_uuids: Vec<uuid::Uuid>,
    /// Egg variables (by environment name) that carry the new ports.
    pub variables: HashMap<String, String>,
    /// Backups whose storage the new node reaches too (S3, a restic repository off the nodes):
    /// they move with the server.
    pub backup_uuids: Vec<uuid::Uuid>,
}

impl Server {
    /// Points the server at `options.node_uuid` with the given allocations, in one transaction:
    /// its allocations on other nodes are released, a transfer in progress is forgotten. Wings is
    /// not called: create the server on the new node afterwards (`post_servers`).
    pub async fn rehome(
        &self,
        state: &crate::State,
        options: RehomeOptions,
    ) -> Result<(), anyhow::Error> {
        let mut transaction = state.database.write().begin().await?;

        let primary = match options.allocation_uuid {
            Some(allocation_uuid) => Some(
                sqlx::query_scalar::<_, uuid::Uuid>(
                    "INSERT INTO server_allocations (server_uuid, allocation_uuid)
                    VALUES ($1, $2)
                    ON CONFLICT DO NOTHING
                    RETURNING uuid",
                )
                .bind(self.uuid)
                .bind(allocation_uuid)
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("the primary allocation is already assigned to a server")
                })?,
            ),
            None => None,
        };

        if !options.allocation_uuids.is_empty() {
            sqlx::query(
                "INSERT INTO server_allocations (server_uuid, allocation_uuid)
                SELECT $1, UNNEST($2::uuid[])
                ON CONFLICT DO NOTHING",
            )
            .bind(self.uuid)
            .bind(&options.allocation_uuids)
            .execute(&mut *transaction)
            .await?;
        }

        sqlx::query(
            "UPDATE servers
            SET node_uuid = $2, allocation_uuid = $3,
                destination_node_uuid = NULL, destination_allocation_uuid = NULL
            WHERE servers.uuid = $1",
        )
        .bind(self.uuid)
        .bind(options.node_uuid)
        .bind(primary)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "DELETE FROM server_allocations
            USING node_allocations
            WHERE node_allocations.uuid = server_allocations.allocation_uuid
                AND server_allocations.server_uuid = $1
                AND node_allocations.node_uuid != $2",
        )
        .bind(self.uuid)
        .bind(options.node_uuid)
        .execute(&mut *transaction)
        .await?;

        if !options.backup_uuids.is_empty() {
            sqlx::query(
                "UPDATE server_backups SET node_uuid = $3
                WHERE server_backups.server_uuid = $1 AND server_backups.uuid = ANY($2)",
            )
            .bind(self.uuid)
            .bind(&options.backup_uuids)
            .bind(options.node_uuid)
            .execute(&mut *transaction)
            .await?;
        }

        if !options.variables.is_empty() {
            sqlx::query(
                "INSERT INTO server_variables (server_uuid, variable_uuid, value)
                SELECT $1, nest_egg_variables.uuid, v.value
                FROM UNNEST($3::text[], $4::text[]) AS v(env_variable, value)
                JOIN nest_egg_variables ON nest_egg_variables.egg_uuid = $2
                    AND nest_egg_variables.env_variable = v.env_variable
                ON CONFLICT (server_uuid, variable_uuid) DO UPDATE SET value = EXCLUDED.value",
            )
            .bind(self.uuid)
            .bind(self.egg.uuid)
            .bind(options.variables.keys().collect::<Vec<_>>())
            .bind(options.variables.values().collect::<Vec<_>>())
            .execute(&mut *transaction)
            .await?;
        }

        let on_mesh =
            crate::tunnel::bump_epoch_if_server_on_mesh(&mut transaction, self.uuid).await?;

        transaction.commit().await?;

        Self::invalidate_cached(&state.database, self.uuid).await;

        if on_mesh {
            crate::tunnel::poke_nodes(&state.database).await;
        }

        // its subdomains point at the new node (docs/features/subdomains.md)
        crate::subdomains::follow(state, self.uuid);

        Ok(())
    }
}
