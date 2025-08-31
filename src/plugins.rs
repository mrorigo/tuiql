/// Plugin system for TUIQL
///
/// This module provides infrastructure for extending TUIQL with external plugins.
/// Plugins are executable programs that communicate with TUIQL via JSON-RPC over stdio.
///
/// The plugin system supports:
/// - Command plugins: Register palette commands
/// - Panel plugins: Custom UI panels
/// - Export plugins: Custom export formats
/// - WASI plugins: Dynamic registration and execution

use crate::config::PluginSpec;
use crate::core::{Result, TuiqlError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

/// Plugin manifest declaration as provided by the plugin.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginManifest {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Plugin capabilities
    pub capabilities: Vec<PluginCapability>,
}

/// Different kinds of plugin capabilities.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum PluginCapability {
    /// Command palette command
    Command {
        /// Command name for palette
        name: String,
        /// Command description
        description: String,
        /// Usage help
        usage: Option<String>,
    },
    /// Custom panel
    Panel {
        /// Panel name
        name: String,
        /// Panel description
        description: String,
        /// Panel category
        category: String,
    },
    /// Custom exporter
    Exporter {
        /// Export format name (e.g., "json", "html")
        format: String,
        /// Format description
        description: String,
        /// File extension
        extension: Option<String>,
    },
}

/// JSON-RPC request structure for plugin communication.
#[derive(Debug, Serialize, Clone)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

/// JSON-RPC response structure.
#[derive(Debug, Deserialize, Clone)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error structure.
#[derive(Debug, Deserialize, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// Represents a loaded plugin with manifest information.
#[derive(Debug, Clone)]
pub struct Plugin {
    /// Plugin specification from config
    pub spec: PluginSpec,
    /// Path to the executable
    pub executable_path: std::path::PathBuf,
    /// Plugin manifest (populated after discovery)
    pub manifest: Option<PluginManifest>,
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
                manifest: None, // Will be populated during discovery
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

    /// Discover plugin capabilities by querying its manifest via JSON-RPC.
    pub fn discover_plugin(&mut self, name: &str) -> Result<()> {
        let plugin = self.get_plugin(name).ok_or_else(|| {
            TuiqlError::Command(format!("Plugin not found: {}", name))
        })?;

        // Clone the plugin for RPC execution (avoiding borrowing conflicts)
        let rpc_plugin = Plugin {
            spec: plugin.spec.clone(),
            executable_path: plugin.executable_path.clone(),
            manifest: None, // Not used for RPC
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "manifest".to_string(),
            params: serde_json::Value::Null,
        };

        let response_str = self.execute_jsonrpc(&rpc_plugin, &request)?;
        let response: JsonRpcResponse = serde_json::from_str(&response_str)
            .map_err(|e| TuiqlError::Command(format!("Failed to parse plugin manifest: {}", e)))?;

        // Find the plugin again for mutation
        if let Some(mut_plugin) = self.get_plugin_mut(name) {
            if let Some(manifest_value) = response.result {
                let manifest: PluginManifest = serde_json::from_value(manifest_value)
                    .map_err(|e| TuiqlError::Command(format!("Invalid plugin manifest: {}", e)))?;
                mut_plugin.manifest = Some(manifest);
            } else if let Some(error) = response.error {
                return Err(TuiqlError::Command(format!(
                    "Plugin manifest discovery failed: {}",
                    error.message
                )));
            }
        }

        Ok(())
    }

    /// Execute a JSON-RPC method on a plugin.
    pub fn execute_jsonrpc(&self, plugin: &Plugin, request: &JsonRpcRequest) -> Result<String> {
        let child = Command::new(&plugin.executable_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| TuiqlError::Command(format!("Failed to spawn plugin: {}", e)))?;

        let request_json = serde_json::to_string(request)
            .map_err(|e| TuiqlError::Command(format!("Failed to serialize request: {}", e)))?;

        child.stdin.as_ref().unwrap().write_all(request_json.as_bytes())?;

        let output = child.wait_with_output()
            .map_err(|e| TuiqlError::Command(format!("Failed to execute plugin: {}", e)))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(stdout.to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(TuiqlError::Command(format!(
                "Plugin JSON-RPC execution failed: {}",
                stderr
            )))
        }
    }

    /// Get a mutable reference to a plugin by name.
    pub fn get_plugin_mut(&mut self, name: &str) -> Option<&mut Plugin> {
        self.plugins.iter_mut().find(|p| p.spec.name == name)
    }

    /// List all discovered capabilities across all plugins.
    pub fn list_capabilities(&self) -> Vec<(String, PluginCapability)> {
        let mut capabilities = Vec::new();
        for plugin in &self.plugins {
            if let Some(manifest) = &plugin.manifest {
                for cap in &manifest.capabilities {
                    capabilities.push((plugin.spec.name.clone(), cap.clone()));
                }
            }
        }
        capabilities
    }

    /// Execute a plugin command with JSON-RPC parameters.
    pub fn execute_command(&self, plugin_name: &str, command_name: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let plugin = self.get_plugin(plugin_name).ok_or_else(|| {
            TuiqlError::Command(format!("Plugin not found: {}", plugin_name))
        })?;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 2,
            method: command_name.to_string(),
            params,
        };

        let response_str = self.execute_jsonrpc(plugin, &request)?;
        let response: JsonRpcResponse = serde_json::from_str(&response_str)
            .map_err(|e| TuiqlError::Command(format!("Failed to parse plugin response: {}", e)))?;

        if let Some(result) = response.result {
            Ok(result)
        } else if let Some(error) = response.error {
            Err(TuiqlError::Command(format!(
                "Plugin command failed: {}",
                error.message
            )))
        } else {
            Err(TuiqlError::Command("Invalid plugin response".to_string()))
        }
    }

    /// Install a plugin from a remote repository (GitHub/GitLab style URLs)
    pub fn install_plugin(&mut self, url: &str, name_override: Option<String>) -> Result<String> {
        // Parse repository URL to extract name
        let name = if let Some(override_name) = name_override {
            override_name
        } else {
            // Extract name from URL: e.g., "gh:user/repo" -> "repo"
            url.split('/').last()
                .unwrap_or("unknown_plugin")
                .trim_end_matches(".git")
                .to_string()
        };

        // Get plugin directory
        let plugin_dir = self.get_plugin_dir()?;
        let plugin_path = plugin_dir.join(&name);

        if plugin_path.exists() {
            return Err(TuiqlError::Command(format!(
                "Plugin '{}' already exists at {}",
                name,
                plugin_path.display()
            )));
        }

        // Clone the repository
        let clone_url = if url.starts_with("gh:") || url.starts_with("github:") {
            format!("https://github.com/{}", &url[3..])
        } else if url.starts_with("gl:") || url.starts_with("gitlab:") {
            format!("https://gitlab.com/{}", &url[3..])
        } else {
            url.to_string()
        };

        std::fs::create_dir_all(&plugin_path)?;

        let status = std::process::Command::new("git")
            .args(&["clone", "--depth", "1", &clone_url, &plugin_path.to_string_lossy()])
            .status()
            .map_err(|e| TuiqlError::Command(format!("Failed to clone repository: {}", e)))?;

        if !status.success() {
            std::fs::remove_dir_all(&plugin_path).ok(); // Clean up on failure
            return Err(TuiqlError::Command("Plugin installation failed".to_string()));
        }

        // Find the executable (look for common patterns)
        let executable_path = self.find_plugin_executable(&plugin_path)?;

        // Add to plugin specs
        let spec = PluginSpec {
            name,
            path: executable_path.to_string_lossy().to_string(),
            description: Some("Auto-installed plugin".to_string()),
        };

        self.plugins.push(Plugin {
            spec: spec.clone(),
            executable_path,
            manifest: None,
        });

        Ok(spec.name)
    }

    /// Get the plugins directory.
    fn get_plugin_dir(&self) -> Result<std::path::PathBuf> {
        let config_dir = crate::config::get_config_dir();
        let plugin_dir = config_dir.join("plugins");
        Ok(plugin_dir)
    }

    /// Find the plugin executable in a plugin directory.
    fn find_plugin_executable(&self, plugin_path: &std::path::Path) -> Result<std::path::PathBuf> {
        // Look for common executable files
        let candidates = ["target/release/plugin", "plugin", "plugin.exe", "main", "bin/plugin"];

        for candidate in &candidates {
            let path = plugin_path.join(candidate);
            if path.exists() && !path.is_dir() {
                // Check if it's executable (on non-Windows systems)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = path.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return Ok(path);
                        }
                    }
                }
                #[cfg(not(unix))]
                return Ok(path);
            }
        }

        Err(TuiqlError::Command(format!(
            "No executable found in plugin directory: {}",
            plugin_path.display()
        )))
    }

    /// List commands from all discovered plugin capabilities.
    pub fn list_plugin_commands(&self) -> Vec<(String, String)> {
        let mut commands = Vec::new();
        for plugin in &self.plugins {
            if let Some(manifest) = &plugin.manifest {
                for capability in &manifest.capabilities {
                    if let PluginCapability::Command { name, description, .. } = capability {
                        commands.push((plugin.spec.name.clone(), name.clone()));
                    }
                }
            }
        }
        commands
    }

    /// List available panels from plugins.
    pub fn list_plugin_panels(&self) -> Vec<(String, String)> {
        let mut panels = Vec::new();
        for plugin in &self.plugins {
            if let Some(manifest) = &plugin.manifest {
                for capability in &manifest.capabilities {
                    if let PluginCapability::Panel { name, description, .. } = capability {
                        panels.push((plugin.spec.name.clone(), name.clone()));
                    }
                }
            }
        }
        panels
    }

    /// List available exporters from plugins.
    pub fn list_plugin_exporters(&self) -> Vec<(String, String)> {
        let mut exporters = Vec::new();
        for plugin in &self.plugins {
            if let Some(manifest) = &plugin.manifest {
                for capability in &manifest.capabilities {
                    if let PluginCapability::Exporter { format, description, .. } = capability {
                        exporters.push((plugin.spec.name.clone(), format.clone()));
                    }
                }
            }
        }
        exporters
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
            manifest: None,
        };
        mgr.plugins.push(plugin);

        let plugins = mgr.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].spec.name, "mock_plugin");
    }

    #[test]
    fn test_discover_plugin_with_manifest() {
        let mut mgr = PluginManager::new();

        // For this test, we'll create a mock executable that echoes a JSON response
        let mut temp_file = NamedTempFile::new().unwrap();
        let manifest_response = r#"{"jsonrpc":"2.0","id":1,"result":{"name":"test","version":"1.0","capabilities":[{"type":"Command","name":"test-cmd","description":"Test command"}]}}"#;
        let script = format!("#!/bin/bash\necho '{}'\n", manifest_response);
        temp_file.write_all(script.as_bytes()).unwrap();

        let temp_path = temp_file.path().to_string_lossy().to_string();

        // Set executable permission (on Unix systems)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp_file.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let spec = PluginSpec {
            name: "test_plugin".to_string(),
            path: temp_path,
            description: Some("Test plugin".to_string()),
        };
        mgr.plugins.push(Plugin {
            spec: spec.clone(),
            executable_path: temp_file.path().to_path_buf(),
            manifest: None,
        });

        let result = mgr.discover_plugin("test_plugin");
        assert!(result.is_ok(), "Discovery failed: {:?}", result.err());

        let plugin = mgr.get_plugin("test_plugin").unwrap();
        assert!(plugin.manifest.is_some(), "Manifest was not populated");
        let manifest = plugin.manifest.as_ref().unwrap();
        assert_eq!(manifest.name, "test");
        assert_eq!(manifest.capabilities.len(), 1);
    }

    #[test]
    fn test_list_plugin_commands() {
        let mut mgr = PluginManager::new();

        // Add plugin with command capability
        let manifest = PluginManifest {
            name: "test_cmd".to_string(),
            version: "1.0".to_string(),
            description: Some("Test command plugin".to_string()),
            capabilities: vec![PluginCapability::Command {
                name: "hello".to_string(),
                description: "Say hello".to_string(),
                usage: None,
            }],
        };

        let spec = PluginSpec {
            name: "test_plugin".to_string(),
            path: "/test/path".to_string(),
            description: None,
        };
        let mut plugin = Plugin {
            spec: spec.clone(),
            executable_path: std::path::PathBuf::from("/test/path"),
            manifest: None,
        };
        plugin.manifest = Some(manifest);
        mgr.plugins.push(plugin);

        let commands = mgr.list_plugin_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], ("test_plugin".to_string(), "hello".to_string()));
    }

    #[test]
    fn test_list_plugin_panels() {
        let mut mgr = PluginManager::new();

        // Add plugin with panel capability
        let manifest = PluginManifest {
            name: "test_panel".to_string(),
            version: "1.0".to_string(),
            description: Some("Test panel plugin".to_string()),
            capabilities: vec![PluginCapability::Panel {
                name: "stats".to_string(),
                description: "Show statistics".to_string(),
                category: "analysis".to_string(),
            }],
        };

        let spec = PluginSpec {
            name: "panel_plugin".to_string(),
            path: "/panel/path".to_string(),
            description: None,
        };
        let mut plugin = Plugin {
            spec: spec.clone(),
            executable_path: std::path::PathBuf::from("/panel/path"),
            manifest: None,
        };
        plugin.manifest = Some(manifest);
        mgr.plugins.push(plugin);

        let panels = mgr.list_plugin_panels();
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0], ("panel_plugin".to_string(), "stats".to_string()));
    }

    #[test]
    fn test_list_plugin_exporters() {
        let mut mgr = PluginManager::new();

        // Add plugin with exporter capability
        let manifest = PluginManifest {
            name: "test_exporter".to_string(),
            version: "1.0".to_string(),
            description: Some("Test exporter plugin".to_string()),
            capabilities: vec![PluginCapability::Exporter {
                format: "csv".to_string(),
                description: "Export as CSV".to_string(),
                extension: Some("csv".to_string()),
            }],
        };

        let spec = PluginSpec {
            name: "export_plugin".to_string(),
            path: "/export/path".to_string(),
            description: None,
        };
        let mut plugin = Plugin {
            spec: spec.clone(),
            executable_path: std::path::PathBuf::from("/export/path"),
            manifest: None,
        };
        plugin.manifest = Some(manifest);
        mgr.plugins.push(plugin);

        let exporters = mgr.list_plugin_exporters();
        assert_eq!(exporters.len(), 1);
        assert_eq!(exporters[0], ("export_plugin".to_string(), "csv".to_string()));
    }

    #[test]
    fn test_list_capabilities_combined() {
        let mut mgr = PluginManager::new();

        // Add plugin with multiple capabilities
        let manifest = PluginManifest {
            name: "multi_cap".to_string(),
            version: "1.0".to_string(),
            description: Some("Multi-capability plugin".to_string()),
            capabilities: vec![
                PluginCapability::Command {
                    name: "cmd1".to_string(),
                    description: "Command 1".to_string(),
                    usage: None,
                },
                PluginCapability::Panel {
                    name: "panel1".to_string(),
                    description: "Panel 1".to_string(),
                    category: "test".to_string(),
                },
            ],
        };

        let spec = PluginSpec {
            name: "multi_plugin".to_string(),
            path: "/multi/path".to_string(),
            description: None,
        };
        let mut plugin = Plugin {
            spec: spec.clone(),
            executable_path: std::path::PathBuf::from("/multi/path"),
            manifest: None,
        };
        plugin.manifest = Some(manifest);
        mgr.plugins.push(plugin);

        let capabilities = mgr.list_capabilities();
        assert_eq!(capabilities.len(), 2);

        // Check that we have both command and panel capabilities
        let command_cap = capabilities.iter().find(|(plugin_name, cap)| {
            *plugin_name == "multi_plugin" && matches!(cap, PluginCapability::Command { .. })
        });
        assert!(command_cap.is_some());

        let panel_cap = capabilities.iter().find(|(plugin_name, cap)| {
            *plugin_name == "multi_plugin" && matches!(cap, PluginCapability::Panel { .. })
        });
        assert!(panel_cap.is_some());
    }
}