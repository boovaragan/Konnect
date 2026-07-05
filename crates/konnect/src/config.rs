use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the kicad-cli binary
    #[serde(default = "default_kicad_cli")]
    pub kicad_cli: String,

    /// Path to the KiCAD binary (for launching the UI)
    #[serde(default = "default_kicad_binary")]
    pub kicad_binary: String,

    /// Default project directory
    #[serde(default)]
    pub project_dir: Option<PathBuf>,

    /// KiCAD IPC socket path (NNG). Auto-detected from KICAD_API_SOCKET env var if empty.
    #[serde(default = "default_ipc_address")]
    #[serde(alias = "ipc_socket_path")]
    pub ipc_address: String,

    /// MCP server transport mode
    #[serde(default)]
    pub transport: TransportMode,

    /// HTTP server bind address (used when transport includes HTTP)
    #[serde(default = "default_http_address")]
    pub http_address: String,

    /// JLCPCB database cache path
    #[serde(default)]
    pub jlcpcb_db_path: Option<PathBuf>,

    /// Log level (error, warn, info, debug, trace)
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TransportMode {
    #[default]
    Stdio,
    Http,
    Both,
}

fn default_kicad_cli() -> String {
    if cfg!(target_os = "windows") {
        "kicad-cli.exe".to_string()
    } else {
        "kicad-cli".to_string()
    }
}

fn default_kicad_binary() -> String {
    if cfg!(target_os = "windows") {
        "kicad.exe".to_string()
    } else {
        "kicad".to_string()
    }
}

fn default_ipc_address() -> String {
    // Empty = auto-detect from KICAD_API_SOCKET env var at runtime
    std::env::var("KICAD_API_SOCKET").unwrap_or_default()
}

fn default_http_address() -> String {
    "127.0.0.1:3000".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Config {
    /// Load config from the default search path.
    pub fn load() -> Result<Self> {
        let config_paths = [
            PathBuf::from("konnect.toml"),
            PathBuf::from("settings.json"),
            dirs_config_path(),
        ];

        for path in &config_paths {
            if path.exists() {
                return Self::load_from(path);
            }
        }

        // No config file found; use defaults
        Ok(Config::default())
    }

    /// Load config from a specific file path. Auto-detects JSON vs TOML by extension.
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "json" => {
                let config: Config = serde_json::from_str(&content)?;
                Ok(config)
            }
            _ => {
                // Default: TOML
                let config: Config = toml::from_str(&content)?;
                Ok(config)
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            kicad_cli: default_kicad_cli(),
            kicad_binary: default_kicad_binary(),
            project_dir: None,
            ipc_address: default_ipc_address(),
            transport: TransportMode::default(),
            http_address: default_http_address(),
            jlcpcb_db_path: None,
            log_level: default_log_level(),
        }
    }
}

fn dirs_config_path() -> PathBuf {
    // Platform-specific config directory
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(appdata).join("konnect").join("config.toml")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("konnect")
            .join("config.toml")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home)
            .join(".config")
            .join("konnect")
            .join("config.toml")
    }
}
