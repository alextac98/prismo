use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
pub struct PluginManifest {
    pub schema_version: u32,
    pub plugin_id: String,
    pub display_name: String,
    pub plugin_version: String,
    pub protocol_version: u32,
    pub language: String,
    #[serde(default)]
    pub config: Option<toml::Value>,
    pub entrypoint: EntrypointConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntrypointConfig {
    pub argv: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DiscoveredPlugin {
    pub manifest_path: PathBuf,
    pub manifest: PluginManifest,
}

pub fn load_plugin_manifest(path: &Path) -> Result<PluginManifest> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read plugin manifest {}", path.display()))?;
    toml::from_str::<PluginManifest>(&raw)
        .with_context(|| format!("failed to parse plugin manifest {}", path.display()))
}

pub fn default_plugin_dir(current_exe: &Path) -> Result<PathBuf> {
    current_exe
        .parent()
        .map(|dir| dir.join("plugins"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "failed to resolve parent directory for {}",
                current_exe.display()
            )
        })
}

pub fn discover_plugins(search_dir: &Path) -> Result<Vec<DiscoveredPlugin>> {
    if !search_dir.exists() {
        return Ok(Vec::new());
    }

    let mut discovered = Vec::new();

    let root_manifest = search_dir.join("prismo-plugin.toml");
    if root_manifest.is_file() {
        discovered.push(DiscoveredPlugin {
            manifest: load_plugin_manifest(&root_manifest)?,
            manifest_path: root_manifest,
        });
    } else {
        for entry in fs::read_dir(search_dir)
            .with_context(|| format!("failed to read plugin directory {}", search_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("failed to read entry in {}", search_dir.display()))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("prismo-plugin.toml");
            if !manifest_path.is_file() {
                continue;
            }

            discovered.push(DiscoveredPlugin {
                manifest: load_plugin_manifest(&manifest_path)?,
                manifest_path,
            });
        }
    }

    discovered.sort_by(|left, right| left.manifest.plugin_id.cmp(&right.manifest.plugin_id));
    validate_unique_plugin_ids(&discovered)?;
    Ok(discovered)
}

fn validate_unique_plugin_ids(discovered: &[DiscoveredPlugin]) -> Result<()> {
    for (index, plugin) in discovered.iter().enumerate() {
        for other in &discovered[index + 1..] {
            if plugin.manifest.plugin_id == other.manifest.plugin_id {
                bail!(
                    "duplicate discovered plugin_id {} in {} and {}",
                    plugin.manifest.plugin_id,
                    plugin.manifest_path.display(),
                    other.manifest_path.display()
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{default_plugin_dir, discover_plugins};

    #[test]
    fn resolves_plugin_dir_relative_to_executable() {
        let dir = default_plugin_dir(Path::new("/tmp/prismo")).expect("plugin dir");
        assert_eq!(dir, PathBuf::from("/tmp/plugins"));
    }

    #[test]
    fn discovers_plugins_one_level_down() {
        let root = unique_temp_path("prismo-discovery");
        let plugin_dir = root.join("example");
        fs::create_dir_all(&plugin_dir).expect("create plugin dir");
        fs::write(
            plugin_dir.join("prismo-plugin.toml"),
            r#"
schema_version = 1
plugin_id = "example"
display_name = "Example"
plugin_version = "0.1.0"
protocol_version = 1
language = "rust"

[entrypoint]
argv = ["./example"]
"#,
        )
        .expect("write manifest");

        let discovered = discover_plugins(&root).expect("discover plugins");
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].manifest.plugin_id, "example");
    }

    #[test]
    fn discovers_single_plugin_directory() {
        let root = unique_temp_path("prismo-single-plugin");
        fs::create_dir_all(&root).expect("create root");
        fs::write(
            root.join("prismo-plugin.toml"),
            r#"
schema_version = 1
plugin_id = "single"
display_name = "Single"
plugin_version = "0.1.0"
protocol_version = 1
language = "cpp"

[entrypoint]
argv = ["./single"]
"#,
        )
        .expect("write manifest");

        let discovered = discover_plugins(&root).expect("discover plugin");
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].manifest.plugin_id, "single");
    }

    #[test]
    fn loads_plugin_config_table() {
        let root = unique_temp_path("prismo-plugin-config");
        fs::create_dir_all(&root).expect("create root");
        fs::write(
            root.join("prismo-plugin.toml"),
            r#"
schema_version = 1
plugin_id = "configured"
display_name = "Configured"
plugin_version = "0.1.0"
protocol_version = 1
language = "rust"

[entrypoint]
argv = ["./configured"]

[config]
ip_address = "10.115.60.123"
sample_rate_hz = 20.0
timeout_ms = 5000
"#,
        )
        .expect("write manifest");

        let discovered = discover_plugins(&root).expect("discover plugin");
        let config = discovered[0]
            .manifest
            .config
            .as_ref()
            .expect("plugin config");

        assert_eq!(config["ip_address"].as_str(), Some("10.115.60.123"));
        assert_eq!(config["sample_rate_hz"].as_float(), Some(20.0));
        assert_eq!(config["timeout_ms"].as_integer(), Some(5000));
    }

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        path.push(format!("{}-{}-{}", prefix, std::process::id(), nanos));
        path
    }
}
