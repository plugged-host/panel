//! plugged.host: endpoints only the Wings fork (plugged-host/wings) has, hand-written so a
//! regeneration from upstream's spec keeps them (docs/wings.md). A stock wings-rs node answers 404.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `GET /api/system/load`.
#[derive(Debug, ToSchema, Deserialize, Serialize, Clone)]
pub struct NodeLoad {
    /// The 1, 5 and 15 minute load averages.
    pub load: [f64; 3],
    pub cpus: u64,
    /// All threads together, 0 to 100.
    pub cpu_percent: f64,
}

#[derive(Debug, ToSchema, Deserialize, Serialize, Clone)]
pub struct NodeProcess {
    pub pid: i64,
    pub ppid: i64,
    pub comm: compact_str::CompactString,
    pub exe: String,
    pub cmd: String,
    /// Percent of one thread.
    pub cpu: f64,
    pub deleted: bool,
    /// Miner strings found in the executable.
    pub tags: Vec<compact_str::CompactString>,
}

#[derive(Debug, ToSchema, Deserialize, Serialize, Clone, Default)]
pub struct NodeContainerNet {
    pub listen: Vec<u16>,
    pub established_ips: u64,
    pub syn_sent_ips: u64,
    /// Remote port → remote addresses the container talks to on it.
    pub ports: std::collections::BTreeMap<u16, u64>,
    pub tx_pps: Option<u64>,
    pub tx_bps: Option<u64>,
}

#[derive(Debug, ToSchema, Deserialize, Serialize, Clone)]
pub struct NodeContainer {
    pub id: compact_str::CompactString,
    /// `None` for a container that is not a running server.
    pub server: Option<uuid::Uuid>,
    pub processes: Vec<NodeProcess>,
    pub net: NodeContainerNet,
}

/// `GET /api/system/processes`.
#[derive(Debug, ToSchema, Deserialize, Serialize, Clone)]
pub struct NodeProcesses {
    pub host: NodeLoad,
    pub containers: Vec<NodeContainer>,
    pub host_processes: Vec<NodeProcess>,
    /// Seconds CPU use was measured over.
    pub span: f64,
}

/// `GET /api/servers/{server}/processes`.
#[derive(Debug, ToSchema, Deserialize, Serialize, Clone)]
pub struct ServerProcesses {
    /// `None` while the server is not running.
    pub container: Option<NodeContainer>,
    pub span: f64,
}

impl super::client::WingsClient {
    async fn fork_json<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
    ) -> Result<T, super::client::ApiHttpError> {
        let response = self
            .request_raw(reqwest::Method::GET, endpoint)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(super::client::ApiHttpError::Reqwest)?;

        let status = response.status();
        if !status.is_success() {
            return Err(super::client::ApiHttpError::Http(
                status,
                response
                    .json::<super::ApiError>()
                    .await
                    .unwrap_or_else(|err| super::ApiError {
                        error: err.to_string().into(),
                    }),
            ));
        }

        response
            .json()
            .await
            .map_err(super::client::ApiHttpError::Reqwest)
    }

    pub async fn get_system_load(&self) -> Result<NodeLoad, super::client::ApiHttpError> {
        self.fork_json("/api/system/load").await
    }

    /// Takes a few seconds when the node was not asked for a while: it measures CPU use first.
    pub async fn get_system_processes(&self) -> Result<NodeProcesses, super::client::ApiHttpError> {
        self.fork_json("/api/system/processes").await
    }

    pub async fn get_servers_server_processes(
        &self,
        server: uuid::Uuid,
    ) -> Result<ServerProcesses, super::client::ApiHttpError> {
        self.fork_json(&format!("/api/servers/{server}/processes"))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape the fork's `src/processes.rs` serializes.
    #[test]
    fn a_process_report_deserializes() {
        let report: NodeProcesses = serde_json::from_str(
            r#"{
                "host": {"load": [0.5, 0.6, 0.7], "cpus": 12, "cpu_percent": 23.4},
                "containers": [{
                    "id": "0123456789ab",
                    "server": "123e4567-e89b-12d3-a456-426614174000",
                    "processes": [{"pid": 10, "ppid": 1, "comm": "java", "exe": "/usr/bin/java",
                        "cmd": "java -jar server.jar", "cpu": 85.5, "deleted": false, "tags": []}],
                    "net": {"listen": [25565], "established_ips": 1, "syn_sent_ips": 0,
                        "ports": {"3333": 2}, "tx_pps": 10, "tx_bps": null}
                }],
                "host_processes": [],
                "span": 60.0
            }"#,
        )
        .expect("deserializes");

        assert_eq!(report.containers[0].net.ports.get(&3333), Some(&2));
        assert_eq!(report.host.cpus, 12);
    }
}
