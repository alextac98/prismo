use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PluginConfig {
    pub plugin_id: String,
    pub manifest: String,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub restart: RestartPolicy,
    #[serde(default)]
    pub config: BTreeMap<String, toml::Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    #[default]
    OnFailure,
    Never,
    Always,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PluginManifest {
    pub schema_version: u32,
    pub plugin_id: String,
    pub display_name: String,
    pub plugin_version: String,
    pub protocol_version: u32,
    pub language: String,
    pub entrypoint: EntrypointConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntrypointConfig {
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InitPayload {
    pub instance_id: String,
    pub plugin_id: String,
    pub config: JsonValue,
}

pub fn load_app_config(path: &Path) -> Result<AppConfig> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read app config {}", path.display()))?;
    let config = toml::from_str::<AppConfig>(&raw)
        .with_context(|| format!("failed to parse app config {}", path.display()))?;

    let mut seen = BTreeMap::new();
    for plugin in &config.plugins {
        if seen.insert(plugin.plugin_id.clone(), ()).is_some() {
            bail!("duplicate plugin_id in app config: {}", plugin.plugin_id);
        }
    }

    Ok(config)
}

pub fn load_plugin_manifest(path: &Path) -> Result<PluginManifest> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read plugin manifest {}", path.display()))?;
    toml::from_str::<PluginManifest>(&raw)
        .with_context(|| format!("failed to parse plugin manifest {}", path.display()))
}

impl PluginConfig {
    pub fn config_json(&self) -> JsonValue {
        JsonValue::Object(
            self.config
                .iter()
                .map(|(key, value)| (key.clone(), toml_to_json(value)))
                .collect::<JsonMap<String, JsonValue>>(),
        )
    }
}

fn enabled_by_default() -> bool {
    true
}

fn toml_to_json(value: &toml::Value) -> JsonValue {
    match value {
        toml::Value::String(value) => JsonValue::String(value.clone()),
        toml::Value::Integer(value) => JsonValue::Number((*value).into()),
        toml::Value::Float(value) => serde_json::Number::from_f64(*value)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        toml::Value::Boolean(value) => JsonValue::Bool(*value),
        toml::Value::Datetime(value) => JsonValue::String(value.to_string()),
        toml::Value::Array(items) => {
            JsonValue::Array(items.iter().map(toml_to_json).collect::<Vec<_>>())
        }
        toml::Value::Table(table) => JsonValue::Object(
            table
                .iter()
                .map(|(key, value)| (key.clone(), toml_to_json(value)))
                .collect::<JsonMap<String, JsonValue>>(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::PluginConfig;

    #[test]
    fn converts_plugin_config_to_json() {
        let config = toml::from_str::<PluginConfig>(
            r#"
plugin_id = "example-rust"
manifest = "./plugins/example-rust/prismo-plugin.toml"

[config]
tick_ms = 150
name = "demo"
"#,
        )
        .expect("plugin config");

        assert_eq!(config.config_json()["tick_ms"], 150);
        assert_eq!(config.config_json()["name"], "demo");
    }
}
