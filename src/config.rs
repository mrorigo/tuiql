use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Top-level configuration structure parsed from a TOML file.
#[derive(Debug, Deserialize)]
pub struct Config {
    pub ui: UIConfig,
    pub keys: Option<KeysConfig>,
    pub sqlite: Option<SqliteConfig>,
}

/// UI-related configuration.
#[derive(Debug, Deserialize)]
pub struct UIConfig {
    pub theme: Option<String>,
    pub show_status_tips: Option<bool>,
}

/// Key binding configuration.
#[derive(Debug, Deserialize)]
pub struct KeysConfig {
    pub run: Option<String>,
    pub run_selection: Option<String>,
    pub vim_mode: Option<bool>,
}

/// SQLite-related configuration.
#[derive(Debug, Deserialize)]
pub struct SqliteConfig {
    pub load_extensions: Option<Vec<String>>,
    pub page_size_hint: Option<u32>,
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
/// let config = load_config("config.toml").expect("Failed to load config");
/// println!("{:?}", config);
/// ```
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    toml::from_str(&content).map_err(|e| e.to_string())
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
        assert_eq!(config.ui.show_status_tips.unwrap(), true);
        if let Some(keys) = config.keys {
            assert_eq!(keys.run.unwrap(), "F5");
            assert_eq!(keys.vim_mode.unwrap(), true);
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
}
