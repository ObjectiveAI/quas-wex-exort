//! Env-driven runtime config (3-struct pattern; mirrors objectiveai-cli).
//!
//! [`EnvConfigBuilder`] is the raw `Envconfig`-derived reader: every field is
//! an `Option<String>` straight from the environment. It lowers into
//! [`ConfigBuilder`] (still all-optional, so `init*` can never fail on a
//! missing var), which finally [`build`](ConfigBuilder::build)s into the
//! [`Config`] the rest of the program uses — unwrapping the required vars and
//! panicking with a clear message if any is absent.

use std::path::PathBuf;

use envconfig::Envconfig;

#[derive(Envconfig)]
struct EnvConfigBuilder {
    /// Root of the CLI's filesystem state tree. Required; unwrapped at
    /// `build()` (we panic if absent).
    #[envconfig(from = "OBJECTIVEAI_STATE_DIR")]
    state_dir: Option<String>,
    /// Dir holding the plugin's binaries, stamped by the host on every spawn.
    /// Required; unwrapped at `build()`.
    #[envconfig(from = "OBJECTIVEAI_BIN_DIR")]
    bin_dir: Option<String>,
    /// Postgres connection URL — the single persistence layer. Required;
    /// unwrapped at `build()`.
    #[envconfig(from = "OBJECTIVEAI_POSTGRES_URL")]
    postgres_url: Option<String>,
    /// This agent instance's hierarchy. Required; unwrapped at `build()`.
    #[envconfig(from = "OBJECTIVEAI_AGENT_INSTANCE_HIERARCHY")]
    objectiveai_agent_instance_hierarchy: Option<String>,
    #[envconfig(from = "OBJECTIVEAI_RESPONSE_ID")]
    objectiveai_response_id: Option<String>,
    #[envconfig(from = "OBJECTIVEAI_RESPONSE_IDS")]
    objectiveai_response_ids: Option<String>,
    #[envconfig(from = "OBJECTIVEAI_AGENT_ID")]
    objectiveai_agent_id: Option<String>,
    #[envconfig(from = "OBJECTIVEAI_AGENT_FULL_ID")]
    objectiveai_agent_full_id: Option<String>,
    #[envconfig(from = "OBJECTIVEAI_AGENT_REMOTE")]
    objectiveai_agent_remote: Option<String>,
}

impl EnvConfigBuilder {
    pub fn build(self) -> ConfigBuilder {
        ConfigBuilder {
            state_dir: self.state_dir,
            bin_dir: self.bin_dir,
            postgres_url: self.postgres_url,
            objectiveai_agent_instance_hierarchy: self.objectiveai_agent_instance_hierarchy,
            objectiveai_response_id: self.objectiveai_response_id,
            objectiveai_response_ids: self.objectiveai_response_ids,
            objectiveai_agent_id: self.objectiveai_agent_id,
            objectiveai_agent_full_id: self.objectiveai_agent_full_id,
            objectiveai_agent_remote: self.objectiveai_agent_remote,
        }
    }
}

#[derive(Default)]
pub struct ConfigBuilder {
    pub state_dir: Option<String>,
    pub bin_dir: Option<String>,
    pub postgres_url: Option<String>,
    pub objectiveai_agent_instance_hierarchy: Option<String>,
    pub objectiveai_response_id: Option<String>,
    pub objectiveai_response_ids: Option<String>,
    pub objectiveai_agent_id: Option<String>,
    pub objectiveai_agent_full_id: Option<String>,
    pub objectiveai_agent_remote: Option<String>,
}

impl Envconfig for ConfigBuilder {
    #[allow(deprecated)]
    fn init() -> Result<Self, envconfig::Error> {
        EnvConfigBuilder::init().map(|e| e.build())
    }

    fn init_from_env() -> Result<Self, envconfig::Error> {
        EnvConfigBuilder::init_from_env().map(|e| e.build())
    }

    fn init_from_hashmap(
        h: &std::collections::HashMap<String, String>,
    ) -> Result<Self, envconfig::Error> {
        EnvConfigBuilder::init_from_hashmap(h).map(|e| e.build())
    }
}

impl ConfigBuilder {
    pub fn build(self) -> Config {
        Config {
            // Required — unwrapped here, after env init. Absence is a hard
            // misconfiguration: panic with a clear message.
            state_dir: PathBuf::from(
                self.state_dir
                    .expect("OBJECTIVEAI_STATE_DIR must be set (the state root)"),
            ),
            bin_dir: PathBuf::from(
                self.bin_dir
                    .expect("OBJECTIVEAI_BIN_DIR must be set (the plugin binaries dir)"),
            ),
            postgres_url: self
                .postgres_url
                .expect("OBJECTIVEAI_POSTGRES_URL must be set"),
            objectiveai_agent_instance_hierarchy: self
                .objectiveai_agent_instance_hierarchy
                .expect("OBJECTIVEAI_AGENT_INSTANCE_HIERARCHY must be set"),
            // Optional — no default; absence is a legitimate `None`.
            objectiveai_response_id: self.objectiveai_response_id,
            objectiveai_response_ids: self.objectiveai_response_ids,
            objectiveai_agent_id: self.objectiveai_agent_id,
            objectiveai_agent_full_id: self.objectiveai_agent_full_id,
            objectiveai_agent_remote: self.objectiveai_agent_remote,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Root of the CLI's filesystem state tree (env `OBJECTIVEAI_STATE_DIR`).
    /// Assumed to already exist. Required (panics if unset).
    pub state_dir: PathBuf,
    /// Dir holding the plugin's binaries (env `OBJECTIVEAI_BIN_DIR`), set by
    /// the host on every spawn. Required (panics if unset).
    pub bin_dir: PathBuf,
    /// Postgres connection URL (env `OBJECTIVEAI_POSTGRES_URL`) — the single
    /// persistence layer. Required.
    pub postgres_url: String,
    /// This agent instance's hierarchy (env
    /// `OBJECTIVEAI_AGENT_INSTANCE_HIERARCHY`). Required.
    pub objectiveai_agent_instance_hierarchy: String,
    /// Single response id (env `OBJECTIVEAI_RESPONSE_ID`). Optional, no default.
    pub objectiveai_response_id: Option<String>,
    /// Multiple response ids (env `OBJECTIVEAI_RESPONSE_IDS`). Optional, no
    /// default.
    pub objectiveai_response_ids: Option<String>,
    /// Default agent id (env `OBJECTIVEAI_AGENT_ID`). Optional, no default.
    pub objectiveai_agent_id: Option<String>,
    /// Agent's fully-qualified id (env `OBJECTIVEAI_AGENT_FULL_ID`). Optional,
    /// no default.
    pub objectiveai_agent_full_id: Option<String>,
    /// Agent's remote ref (env `OBJECTIVEAI_AGENT_REMOTE`). Optional, no
    /// default.
    pub objectiveai_agent_remote: Option<String>,
}

impl Config {
    /// The state root (env `OBJECTIVEAI_STATE_DIR`). All state files live
    /// directly under it; assumed to already exist.
    pub fn state_dir(&self) -> PathBuf {
        self.state_dir.clone()
    }
}

/// Build the runtime config from the process environment.
pub fn load_config() -> Config {
    ConfigBuilder::init_from_env().unwrap_or_default().build()
}
