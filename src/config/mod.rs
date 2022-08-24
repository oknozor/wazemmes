use crate::config::keybinding::{Action, KeyBinding, Modifier};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use xkbcommon::xkb;

pub mod keybinding;

pub static CONFIG: Lazy<WazemmesConfig> = Lazy::new(WazemmesConfig::default);

#[derive(Debug, Deserialize, Serialize)]
pub struct WazemmesConfig {
    pub gaps: u32,
    pub keybindings: Vec<KeyBinding>,
}

impl WazemmesConfig {
    pub fn get() -> eyre::Result<WazemmesConfig> {
        let file = dirs::home_dir()
            .expect("$HOME should be set")
            .join(".config/wazemmes/config.ron");

        let file = fs::read_to_string(file)?;
        let config = ron::from_str(&file)?;
        Ok(config)
    }
}

impl Default for WazemmesConfig {
    fn default() -> Self {
        Self {
            gaps: 14,
            keybindings: vec![
                KeyBinding {
                    modifiers: HashSet::from([Modifier::Alt]),
                    key: xkb::KEY_t,
                    action: Action::Run {
                        env: vec![],
                        command: "alacritty".to_string(),
                    },
                },
                KeyBinding {
                    modifiers: HashSet::from([Modifier::Alt]),
                    key: xkb::KEY_g,
                    action: Action::Run {
                        env: vec![("WGPU_BACKEND".into(), "vulkan".into())],
                        command: "onagre".to_string(),
                    },
                },
            ],
        }
    }
}
