use std::collections::HashMap;
use std::path::PathBuf;

use compact_str::CompactString;
use serde::{Deserialize, Serialize};

use crate::permission::{PermissionConfig, PermissionConfigs};
use crate::session::storage;

#[cfg(feature = "mcp")]
use crate::extras::mcp::config::McpServerConfig;

#[cfg(feature = "acp")]
use crate::extras::acp::config::AcpServerConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickModelConfig {
    pub provider: CompactString,
    pub model: CompactString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiStyle {
    Responses,
    Completions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub provider_type: CompactString,
    pub base_url: String,
    pub api_key_env: Option<CompactString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub danger_accept_invalid_certs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_style: Option<ApiStyle>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorsConfig {
    pub chat_background: Option<CompactString>,
    pub input_background: Option<CompactString>,
    pub status_background: Option<CompactString>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_tools: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_context_files: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_recent_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_agent_turns: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_text_file_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_providers: Option<HashMap<String, CustomProviderConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "permission-regex")]
    pub permission_regex: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "permission-allow")]
    pub permission_allow: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "permission-ask")]
    pub permission_ask: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "permission-deny")]
    pub permission_deny: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restrictive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_all: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yolo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_all_mcp_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_permission_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_tool_details: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_prompt: Option<CompactString>,
    #[cfg(feature = "git-worktree")]
    #[serde(skip_serializing_if = "Option::is_none", rename = "wt-auto-merge")]
    pub wt_auto_merge: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_keys: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quick_models: Option<std::collections::HashMap<String, QuickModelConfig>>,
    #[cfg(feature = "mcp")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,

    #[cfg(feature = "acp")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acp_servers: Option<HashMap<String, AcpServerConfig>>,
    #[cfg(feature = "acp")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acp_host: Option<String>,
    #[cfg(feature = "acp")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acp_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<ColorsConfig>,
}

impl Config {
    pub fn custom_providers_map(&self) -> HashMap<String, CustomProviderConfig> {
        self.custom_providers.clone().unwrap_or_default()
    }

    pub fn resolve_context_window(&self) -> u64 {
        self.context_window.unwrap_or(128_000)
    }

    pub fn resolve_reserve_tokens(&self) -> u64 {
        self.reserve_tokens.unwrap_or(16_384)
    }

    pub fn resolve_keep_recent_tokens(&self) -> u64 {
        self.keep_recent_tokens.unwrap_or(20_000)
    }

    pub fn resolve_compact_enabled(&self) -> bool {
        self.compact_enabled.unwrap_or(true)
    }

    pub fn build_permission_config(&self) -> PermissionConfigs {
        let glob: PermissionConfig = self
            .permission
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let regex: PermissionConfig = self
            .permission_regex
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let mut perm_configs = PermissionConfigs { glob, regex };

        if let Some(allow) = &self.permission_allow {
            perm_configs.glob.allow_entries = Some(allow.clone());
        }
        if let Some(ask) = &self.permission_ask {
            perm_configs.glob.ask_entries = Some(ask.clone());
        }
        if let Some(deny) = &self.permission_deny {
            perm_configs.glob.deny_entries = Some(deny.clone());
        }

        perm_configs
    }
}

fn resolve_config_path() -> PathBuf {
    // ZS_CONFIG_DIR env var takes highest priority
    if let Some(dir) = std::env::var_os("ZS_CONFIG_DIR") {
        let dir = PathBuf::from(dir);
        let toml = dir.join("config.toml");
        let json = dir.join("config.json");
        if toml.exists() {
            return toml;
        }
        if json.exists() {
            return json;
        }
        return toml;
    }

    // XDG config dir (~/.config/zerostack on Linux) takes second priority
    if let Some(config_dir) = dirs::config_dir() {
        let dir = config_dir.join("zerostack");
        let toml = dir.join("config.toml");
        let json = dir.join("config.json");
        if toml.exists() {
            return toml;
        }
        if json.exists() {
            return json;
        }
    }

    // Data dir (~/.local/share/zerostack) is the final fallback
    let dir = storage::data_dir();
    let toml = dir.join("config.toml");
    let json = dir.join("config.json");
    if toml.exists() {
        toml
    } else if json.exists() {
        json
    } else {
        toml
    }
}

pub fn config_file_path() -> PathBuf {
    resolve_config_path()
}

pub fn quick_models_map(cfg: &Config) -> HashMap<String, QuickModelConfig> {
    cfg.quick_models.clone().unwrap_or_default()
}

pub fn save_quick_model(name: &str, provider: &str, model: &str) -> std::io::Result<()> {
    let path = resolve_config_path();
    let mut cfg: Config = if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => toml::from_str(&content).unwrap_or_default(),
            _ => serde_json::from_str(&content).unwrap_or_default(),
        }
    } else {
        Config::default()
    };

    let quick_models = cfg.quick_models.get_or_insert_with(HashMap::new);
    quick_models.insert(
        name.to_string(),
        QuickModelConfig {
            provider: CompactString::new(provider),
            model: CompactString::new(model),
        },
    );

    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid config path")
    })?;
    std::fs::create_dir_all(parent)?;
    match path.extension().and_then(|e| e.to_str()) {
        Some("toml") => {
            let content = toml::to_string(&cfg)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            std::fs::write(&path, content)?;
        }
        _ => std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?,
    }
    Ok(())
}

pub fn load() -> Config {
    let path = resolve_config_path();
    #[allow(unused_mut)]
    let mut cfg: Config = if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let default = Config::default();
        if path.extension().and_then(|e| e.to_str()) == Some("toml")
            && let Ok(content) = toml::to_string(&default)
        {
            std::fs::write(&path, content).ok();
        }
        default
    } else {
        let content = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!(
                "error: failed to read config file ({}): {}\n\
                 Fix the file or remove it to use defaults.",
                path.display(),
                e,
            );
            std::process::exit(1);
        });
        match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!(
                    "error: {} is not a valid config: {}\n\
                     Fix the file or remove it to use defaults.",
                    path.display(),
                    e,
                );
                std::process::exit(1);
            }),
            _ => serde_json::from_str(&content).unwrap_or_else(|e| {
                eprintln!(
                    "error: {} is not a valid config: {}\n\
                     Fix the file or remove it to use defaults.",
                    path.display(),
                    e,
                );
                std::process::exit(1);
            }),
        }
    };

    #[cfg(feature = "mcp")]
    if cfg.mcp_servers.is_none() {
        let mut headers = HashMap::new();
        if let Ok(key) = std::env::var("EXA_API_KEY") {
            headers.insert("x-api-key".to_string(), key);
        }
        let mut defaults = HashMap::new();
        defaults.insert(
            "Exa Web Search".to_string(),
            McpServerConfig::Url {
                url: "https://mcp.exa.ai/mcp".to_string(),
                headers,
            },
        );
        cfg.mcp_servers = Some(defaults);
    }

    cfg
}
