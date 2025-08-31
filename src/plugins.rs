/// Plugin system for TUIQL
///
/// This module provides infrastructure for extending TUIQL with external plugins.
/// Plugins are executable programs that communicate with TUIQL via JSON-RPC over stdio.

use crate::config::PluginSpec;
use crate::core::{Result, TuiqlError};
use std::fs;
use std::process::{Command, Stdio};

/// Represents a loaded plugin.
#[derive(Debug, Clone)]
pub struct Plugin {
    /// Plugin specification from config
    pub spec: PluginSpec,
    /// Path to the executable
    pub executable_path: std::path::PathBuf,
}

/// PluginManager manages the discovery, loading, and execution of plugins.
pub struct PluginManager {
    plugins: Vec<Plugin>,
}

impl PluginManager {
    /// Create a new PluginManager with no plugins loaded.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Load plugins from the plugin specifications.
    pub fn load_plugins(&mut self, specs: &[PluginSpec]) -> Result<()> {
        for spec in specs {
            let executable_path = std::path::PathBuf::from(&spec.path);

            if !executable_path.exists() {
                return Err(TuiqlError::Command(format!(
                    "Plugin executable not found: {}",
                    spec.path
                )));
            }

            if ! fs::metadata(&executable_path)?.is_file() {
                return Err(TuiqlError::Command(format!(
                    "Plugin path is not a file: {}",
                    spec.path
                )));
            }

            let plugin = Plugin {
                spec: spec.clone(),
                executable_path: executable_path.canonicalize()?,
            };
            self.plugins.push(plugin);
        }
        Ok(())
    }

    /// Get a plugin by name.
    pub fn get_plugin(&self, name: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.spec.name == name)
    }

    /// List all loaded plugins.
    pub fn list_plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    /// Execute a plugin with the given arguments.
    ///
    /// The plugin is expected to accept JSON input on stdin and output JSON response.
    pub fn execute_plugin(&self, name: &str, args: &[String]) -> Result<String> {
        let plugin = self.get_plugin(name).ok_or_else(|| {
            TuiqlError::Command(format!("Plugin not found: {}", name))
        })?;

        let child = Command::new(&plugin.executable_path)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // For now, no input to stdin, just read stdout
        let output = child.wait_with_output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(stdout.to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(TuiqlError::Command(format!(
                "Plugin execution failed: {}",
                stderr
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginSpec;
    use tempfile::NamedTempFile;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_plugin_manager_creation() {
        let mgr = PluginManager::new();
        assert!(mgr.plugins.is_empty());
    }

    #[test]
    fn test_get_plugin_not_found() {
        let mgr = PluginManager::new();
        assert!(mgr.get_plugin("nonexistent").is_none());
    }

    #[test]
    fn test_load_plugins_with_valid_executable() {
        let mut mgr = PluginManager::new();

        // Create a temporary executable (shell script)
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"#!/bin/bash\necho 'Plugin output'\n").unwrap();
        let temp_path = temp_file.path().to_string_lossy().to_string();

        // Set executable permission (on Unix systems)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp_file.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let specs = vec![PluginSpec {
            name: "test_plugin".to_string(),
            path: temp_path,
            description: Some("Test plugin".to_string()),
        }];

        let result = mgr.load_plugins(&specs);
        assert!(result.is_ok());
        assert_eq!(mgr.plugins.len(), 1);
        assert_eq!(mgr.plugins[0].spec.name, "test_plugin");
    }

    #[test]
    fn test_load_plugins_with_nonexistent_executable() {
        let mut mgr = PluginManager::new();

        let specs = vec![PluginSpec {
            name: "nonexistent_plugin".to_string(),
            path: "/nonexistent/path".to_string(),
            description: None,
        }];

        let result = mgr.load_plugins(&specs);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_plugin_success() {
        let mut mgr = PluginManager::new();

        // Create a temporary executable
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"#!/bin/bash\necho 'Success output'\n").unwrap();
        let temp_path = temp_file.path().to_string_lossy().to_string();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp_file.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let specs = vec![PluginSpec {
            name: "success_plugin".to_string(),
            path: temp_path,
            description: None,
        }];

        mgr.load_plugins(&specs).unwrap();

        let result = mgr.execute_plugin("success_plugin", &vec![]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "Success output");
    }

    #[test]
    fn test_execute_plugin_not_found() {
        let mgr = PluginManager::new();

        let result = mgr.execute_plugin("missing_plugin", &vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_plugins() {
        let mut mgr = PluginManager::new();

        // Add a mock plugin directly
        let spec = PluginSpec {
            name: "mock_plugin".to_string(),
            path: "/mock/path".to_string(),
            description: None,
        };

        let plugin = Plugin {
            spec,
            executable_path: std::path::PathBuf::from("/mock/path"),
        };
        mgr.plugins.push(plugin);

        let plugins = mgr.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].spec.name, "mock_plugin");
    }
}