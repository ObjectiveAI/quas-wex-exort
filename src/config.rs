//! Env-driven runtime config (3-struct pattern; mirrors objectiveai-cli).
//!
//! [`EnvConfigBuilder`] is the raw `Envconfig`-derived reader: every field is
//! an `Option<String>` straight from the environment. It lowers into
//! [`ConfigBuilder`] (still all-optional, so `init*` can never fail on a
//! missing var), which finally [`build`](ConfigBuilder::build)s into the
//! [`Config`] the rest of the program uses — unwrapping the one required var
//! (`OBJECTIVEAI_STATE_DIR`) and panicking with a clear message if it is absent.

use std::path::PathBuf;

use envconfig::Envconfig;

#[derive(Envconfig)]
struct EnvConfigBuilder {
    /// Root of the CLI's filesystem state tree. Required; unwrapped at
    /// `build()` (we panic if absent).
    #[envconfig(from = "OBJECTIVEAI_STATE_DIR")]
    state_dir: Option<String>,
    /// Dir holding the plugin's binaries, stamped by the host on every spawn.
    /// Optional — the daemon never reads it.
    #[envconfig(from = "OBJECTIVEAI_BIN_DIR")]
    bin_dir: Option<String>,
    /// Postgres connection URL. Optional — the daemon never reads it.
    #[envconfig(from = "OBJECTIVEAI_POSTGRES_URL")]
    postgres_url: Option<String>,
    /// This agent instance's hierarchy. Optional — see [`Config`] field doc.
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
            // The only hard requirement: the state root (the lockfile dir lives
            // under it). Everything else is optional because this plugin never
            // reads it: `bin_dir`/`postgres_url`/the agent vars *are* stamped on
            // the process by the host (the daemon inherits the launching agent's
            // env), but the daemon is a per-state singleton, so any agent
            // identity it captured is just whoever happened to launch it first.
            // The identity that matters is per-request and read from request
            // headers (the AIH, the arguments), not from this process env.
            state_dir: PathBuf::from(
                self.state_dir
                    .expect("OBJECTIVEAI_STATE_DIR must be set (the state root)"),
            ),
            bin_dir: self.bin_dir.map(PathBuf::from),
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

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Root of the CLI's filesystem state tree (env `OBJECTIVEAI_STATE_DIR`).
    /// Assumed to already exist. The only required field (panics if unset).
    pub state_dir: PathBuf,
    /// Dir holding the plugin's binaries (env `OBJECTIVEAI_BIN_DIR`). Optional.
    pub bin_dir: Option<PathBuf>,
    /// Postgres connection URL (env `OBJECTIVEAI_POSTGRES_URL`). Optional.
    pub postgres_url: Option<String>,
    /// This agent instance's hierarchy (env
    /// `OBJECTIVEAI_AGENT_INSTANCE_HIERARCHY`). Optional — the daemon is a
    /// per-state singleton, so this is just the launching agent's AIH and is
    /// unused; per-request identity is read from headers.
    pub objectiveai_agent_instance_hierarchy: Option<String>,
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
