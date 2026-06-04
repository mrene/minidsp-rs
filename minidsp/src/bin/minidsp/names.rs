use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct DeviceNames {
    #[serde(default)]
    pub inputs: IndexMap<String, usize>,
    #[serde(default)]
    pub outputs: IndexMap<String, usize>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct NamesConfig {
    #[serde(default)]
    pub device: HashMap<String, DeviceNames>,
}

impl NamesConfig {
    fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("minidsp").join("names.toml"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path().ok_or_else(|| anyhow!("cannot determine config directory"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn for_device(&self, serial: u32) -> DeviceNames {
        self.device
            .get(&serial.to_string())
            .cloned()
            .unwrap_or_default()
    }

    pub fn for_device_mut(&mut self, serial: u32) -> &mut DeviceNames {
        self.device.entry(serial.to_string()).or_default()
    }

    pub fn validate_name(name: &str) -> Result<()> {
        if name.parse::<usize>().is_ok() {
            return Err(anyhow!("name cannot be a number (would shadow an index)"));
        }
        Ok(())
    }
}

fn resolve(map: &IndexMap<String, usize>, kind: &str, name_or_index: &str) -> Result<usize> {
    if let Ok(idx) = name_or_index.parse::<usize>() {
        return Ok(idx);
    }
    map.get(name_or_index)
        .copied()
        .ok_or_else(|| anyhow!("unknown {} name: '{}'", kind, name_or_index))
}

fn label_for(map: &IndexMap<String, usize>, index: usize) -> String {
    map.iter()
        .find(|(_, &idx)| idx == index)
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| index.to_string())
}

impl DeviceNames {
    pub fn resolve_input(&self, name_or_index: &str) -> Result<usize> {
        resolve(&self.inputs, "input", name_or_index)
    }

    pub fn resolve_output(&self, name_or_index: &str) -> Result<usize> {
        resolve(&self.outputs, "output", name_or_index)
    }

    pub fn label_for_input(&self, index: usize) -> String {
        label_for(&self.inputs, index)
    }

    pub fn label_for_output(&self, index: usize) -> String {
        label_for(&self.outputs, index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_device_names() -> DeviceNames {
        let mut dn = DeviceNames::default();
        dn.inputs.insert("left".into(), 0);
        dn.inputs.insert("right".into(), 1);
        dn.outputs.insert("left".into(), 0);
        dn.outputs.insert("right".into(), 1);
        dn.outputs.insert("sub_left".into(), 2);
        dn.outputs.insert("main_l".into(), 0);
        dn
    }

    #[test]
    fn resolve_by_index() {
        let dn = sample_device_names();
        assert_eq!(dn.resolve_input("0").unwrap(), 0);
        assert_eq!(dn.resolve_output("3").unwrap(), 3);
    }

    #[test]
    fn resolve_by_name() {
        let dn = sample_device_names();
        assert_eq!(dn.resolve_input("left").unwrap(), 0);
        assert_eq!(dn.resolve_output("sub_left").unwrap(), 2);
    }

    #[test]
    fn resolve_unknown_name() {
        let dn = sample_device_names();
        assert!(dn.resolve_input("bogus").is_err());
        assert!(dn.resolve_output("bogus").is_err());
    }

    #[test]
    fn validate_rejects_numeric() {
        assert!(NamesConfig::validate_name("3").is_err());
        assert!(NamesConfig::validate_name("42").is_err());
    }

    #[test]
    fn validate_accepts_text() {
        assert!(NamesConfig::validate_name("left").is_ok());
        assert!(NamesConfig::validate_name("sub_3").is_ok());
    }

    #[test]
    fn label_uses_first_in_order() {
        let dn = sample_device_names();
        assert_eq!(dn.label_for_output(0), "left");
    }

    #[test]
    fn label_falls_back_to_index() {
        let dn = sample_device_names();
        assert_eq!(dn.label_for_output(9), "9");
        assert_eq!(dn.label_for_input(5), "5");
    }

    #[test]
    fn for_device_returns_empty_for_unknown() {
        let cfg = NamesConfig::default();
        let dn = cfg.for_device(999999);
        assert!(dn.inputs.is_empty());
        assert!(dn.outputs.is_empty());
    }

    #[test]
    fn for_device_mut_creates_entry() {
        let mut cfg = NamesConfig::default();
        cfg.for_device_mut(123).inputs.insert("left".into(), 0);
        assert_eq!(cfg.for_device(123).resolve_input("left").unwrap(), 0);
    }
}
