use crate::core::{Result, TuiqlError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use dirs;

/// Top-level configuration structure parsed from a TOML file.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub ui: UIConfig,
    pub keys: Option<KeysConfig>,
    pub sqlite: Option<SqliteConfig>,
    pub plugins: Option<PluginsConfig>,
}

/// UI-related configuration.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UIConfig {
    pub theme: Option<String>,
    pub show_status_tips: Option<bool>,
}

/// Key binding configuration.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct KeysConfig {
    pub run: Option<String>,
    pub run_selection: Option<String>,
    pub vim_mode: Option<bool>,
}

/// SQLite-related configuration.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SqliteConfig {
    pub load_extensions: Option<Vec<String>>,
    pub page_size_hint: Option<u32>,
}

/// Plugin-related configuration.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[derive(Default)]
pub struct PluginsConfig {
    pub enabled: Vec<PluginSpec>,
}

/// Specification for a plugin.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginSpec {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UIConfig::default(),
            keys: Some(KeysConfig::default()),
            sqlite: Some(SqliteConfig::default()),
            plugins: Some(PluginsConfig::default()),
        }
    }
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            theme: Some("dark".to_string()),
            show_status_tips: Some(true),
        }
    }
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            run: Some("F5".to_string()),
            run_selection: Some("S-F5".to_string()),
            vim_mode: Some(true),
        }
    }
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            load_extensions: None,
            page_size_hint: Some(4096),
        }
    }
}


/// Loads configuration from a TOML file at the given path.
///
/// # Arguments
///
/// * `path` - The file path to the TOML configuration file.
///
/// # Example
///
/// ```
/// use tuiql::config::load_config;
/// use std::fs;
/// use tempfile::NamedTempFile;
///
/// // Create a temporary config file for the example
/// let config_content = r#"
/// [ui]
/// theme = "dark"
/// show_status_tips = true
/// "#;
/// let temp_file = NamedTempFile::with_suffix(".toml").unwrap();
/// fs::write(&temp_file.path(), config_content).unwrap();
///
/// let config = load_config(&temp_file.path()).expect("Failed to load config");
/// println!("{:?}", config);
/// ```
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path_ref = path.as_ref().to_path_buf();
    let content = fs::read_to_string(&path_ref)
        .map_err(|e| TuiqlError::Config(format!("Failed to read config file {:?}: {}", path_ref, e)))?;

    toml::from_str(&content)
        .map_err(|e| TuiqlError::Config(format!("Failed to parse config TOML: {}", e)))
}

/// Returns the path to the tuiql configuration directory using XDG Base Directory Specification.
/// Falls back to ~/.config/tuiql if XDG variables are not set.
pub fn get_config_dir() -> PathBuf {
    if let Some(mut config_dir) = dirs::config_dir() {
        config_dir.push("tuiql");
        config_dir
    } else {
        // Fallback to ~/.config/tuiql
        let mut home_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        home_dir.push(".config");
        home_dir.push("tuiql");
        home_dir
    }
}

/// Returns the path to the tuiql configuration file.
pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.toml")
}

/// Returns the path to the tuiql data directory using XDG Base Directory Specification.
/// Falls back to ~/.local/share/tuiql if XDG variables are not set.
/// This is used for storage.db and other application data.
pub fn get_data_dir() -> PathBuf {
    if let Some(mut data_dir) = dirs::data_dir() {
        data_dir.push("tuiql");
        data_dir
    } else {
        // Fallback to ~/.local/share/tuiql
        let mut home_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        home_dir.push(".local");
        home_dir.push("share");
        home_dir.push("tuiql");
        home_dir
    }
}

/// Returns the path to the storage database file.
pub fn get_storage_path() -> PathBuf {
    get_data_dir().join("storage.db")
}

/// Loads the configuration from the standard XDG location.
/// Creates the config directory and default config file if they don't exist.
pub fn load_or_create_config() -> Result<Config> {
    let config_dir = get_config_dir();
    let config_path = get_config_path();

    // Try to load existing config
    if config_path.exists() {
        return load_config(&config_path);
    }

    // Create directory and default config
    fs::create_dir_all(&config_dir)
        .map_err(|e| TuiqlError::Config(format!("Failed to create config directory {:?}: {}", config_dir, e)))?;

    let default_config = Config::default();
    let toml_content = toml::to_string_pretty(&default_config)
        .map_err(|e| TuiqlError::Config(format!("Failed to serialize default config: {}", e)))?;

    fs::write(&config_path, &toml_content)
        .map_err(|e| TuiqlError::Config(format!("Failed to write default config file {:?}: {}", config_path, e)))?;

    Ok(default_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONFIG: &str = r#"
[ui]
theme = "dark"
show_status_tips = true

[keys]
run = "F5"
run_selection = "S-F5"
vim_mode = true

[sqlite]
load_extensions = ["/usr/lib/sqlite3/fts5.so"]
page_size_hint = 4096
"#;

    #[test]
    fn test_load_config_from_str() {
        let config: Config = toml::from_str(SAMPLE_CONFIG).expect("Failed to parse sample config");
        assert_eq!(config.ui.theme.unwrap(), "dark");
        assert!(config.ui.show_status_tips.unwrap());
        if let Some(keys) = config.keys {
            assert_eq!(keys.run.unwrap(), "F5");
            assert!(keys.vim_mode.unwrap());
        } else {
            panic!("Keys configuration not found");
        }
        if let Some(sqlite) = config.sqlite {
            assert_eq!(sqlite.page_size_hint.unwrap(), 4096);
            let load_extensions = sqlite.load_extensions.unwrap();
            assert_eq!(load_extensions.len(), 1);
            assert_eq!(load_extensions[0], "/usr/lib/sqlite3/fts5.so");
        } else {
            panic!("SQLite configuration not found");
        }
    }

    #[test]
    fn test_config_defaults() {
        let default_config = Config::default();
        assert_eq!(default_config.ui.theme.unwrap(), "dark");
        assert!(default_config.ui.show_status_tips.unwrap());
        let keys = default_config.keys.unwrap();
        assert_eq!(keys.run.unwrap(), "F5");
        assert!(keys.vim_mode.unwrap());
        let sqlite = default_config.sqlite.unwrap();
        assert_eq!(sqlite.page_size_hint.unwrap(), 4096);
        assert!(sqlite.load_extensions.is_none());
    }

    #[test]
    fn test_get_config_dir() {
        let config_dir = get_config_dir();
        assert!(config_dir.ends_with("tuiql"));
        // Should contain either .config/tuiql or similar
        let config_dir_str = config_dir.to_string_lossy();
        assert!(config_dir_str.contains("tuiql"));
    }
}
