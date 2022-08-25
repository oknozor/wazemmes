use crate::inputs::handlers::Direction;
use crate::inputs::KeyAction;
use serde::{Deserialize, Serialize, Serializer};
use smithay::wayland::seat;
use std::collections::HashSet;
use std::hash::Hash;
use xkbcommon::xkb;
use xkbcommon::xkb::Keysym;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyBinding {
    pub modifiers: HashSet<Modifier>,
    #[serde(serialize_with = "serialize_key")]
    #[serde(deserialize_with = "deserialize_key")]
    pub key: Keysym,
    pub action: Action,
}

impl KeyBinding {
    pub fn match_action(&self, modifiers: seat::ModifiersState, key: Keysym) -> Option<Action> {
        let state: seat::ModifiersState = self.into();
        if state == modifiers && key == self.key {
            Some(self.action.clone())
        } else {
            None
        }
    }
}

impl Into<seat::ModifiersState> for &KeyBinding {
    fn into(self) -> seat::ModifiersState {
        seat::ModifiersState {
            ctrl: self.modifiers.contains(&Modifier::Ctrl),
            alt: self.modifiers.contains(&Modifier::Alt),
            shift: self.modifiers.contains(&Modifier::Shift),
            caps_lock: self.modifiers.contains(&Modifier::CapsLock),
            logo: self.modifiers.contains(&Modifier::Logo),
            num_lock: self.modifiers.contains(&Modifier::NumLock),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Action {
    MoveWindowLeft,
    MoveWindowRight,
    MoveWindowDown,
    MoveWindowUp,
    MoveContainerLeft,
    MoveContainerRight,
    MoveContainerDown,
    MoveContainerUp,
    MoveFocusLeft,
    ToggleFullScreenWindow,
    ToggleFullScreenContainer,
    MoveFocusRight,
    MoveFocusDown,
    MoveFocusUp,
    MoveToWorkspace(u8),
    LayoutVertical,
    LayoutHorizontal,
    ToggleFloating,
    Run {
        env: Vec<(String, String)>,
        command: String,
    },
    Close,
    Quit,
}

impl Into<KeyAction> for Action {
    fn into(self) -> KeyAction {
        match self {
            Action::MoveWindowLeft => KeyAction::MoveWindow(Direction::Left),
            Action::MoveWindowRight => KeyAction::MoveWindow(Direction::Right),
            Action::MoveWindowDown => KeyAction::MoveWindow(Direction::Down),
            Action::MoveWindowUp => KeyAction::MoveWindow(Direction::Up),
            Action::MoveContainerLeft => KeyAction::MoveContainer(Direction::Left),
            Action::MoveContainerRight => KeyAction::MoveContainer(Direction::Right),
            Action::MoveContainerDown => KeyAction::MoveContainer(Direction::Down),
            Action::MoveContainerUp => KeyAction::MoveContainer(Direction::Up),
            Action::MoveFocusLeft => KeyAction::MoveFocus(Direction::Left),
            Action::MoveFocusRight => KeyAction::MoveFocus(Direction::Right),
            Action::MoveFocusDown => KeyAction::MoveFocus(Direction::Down),
            Action::MoveFocusUp => KeyAction::MoveFocus(Direction::Up),
            Action::MoveToWorkspace(num) => KeyAction::MoveToWorkspace(num),
            Action::LayoutVertical => KeyAction::LayoutVertical,
            Action::LayoutHorizontal => KeyAction::LayoutHorizontal,
            Action::ToggleFloating => KeyAction::ToggleFloating,
            Action::Run { command, env } => KeyAction::Run(command, env),
            Action::Close => KeyAction::Close,
            Action::Quit => KeyAction::Quit,
            Action::ToggleFullScreenWindow => KeyAction::ToggleFullScreenWindow,
            Action::ToggleFullScreenContainer => KeyAction::ToggleFullScreenContainer,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct EnvVar {
    key: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Logo,
    CapsLock,
    NumLock,
}

fn serialize_key<S>(key: &Keysym, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let name = xkb::keysym_get_name(*key);
    serializer.serialize_str(&name)
}

#[allow(non_snake_case)]
fn deserialize_key<'de, D>(deserializer: D) -> Result<Keysym, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected};

    let name = String::deserialize(deserializer)?;
    match xkb::keysym_from_name(&name, xkb::KEY_NoSymbol) {
        xkb::KEY_NoSymbol => match xkb::keysym_from_name(&name, xkb::KEYSYM_CASE_INSENSITIVE) {
            xkb::KEY_NoSymbol => Err(<D::Error as Error>::invalid_value(
                Unexpected::Str(&name),
                &"Invalid keysym",
            )),
            key => {
                slog_scope::warn!(
                    "Key-Binding '{}' only matched case insensitive for {:?}",
                    name,
                    xkb::keysym_get_name(key)
                );
                Ok(key)
            }
        },
        key => Ok(key),
    }
}

#[cfg(test)]
mod test {
    use crate::config::keybinding::{deserialize_key, Action, EnvVar, KeyBinding, Modifier};
    use crate::WazemmesConfig;
    use indoc::indoc;
    use speculoos::prelude::*;
    use std::collections::HashSet;
    use xkbcommon::xkb;

    #[test]
    fn should_deserialize_keybindings() {
        let keys = indoc! {
            r#"
            modifiers = ["Ctrl", "Alt"]
            key = "A"
            command = "alacritty"
            "#
        };

        let binding = ron::from_str::<KeyBinding>(&keys);

        assert_that!(binding).is_ok();
        let binding = binding.unwrap();

        assert_that!(&binding.modifiers.iter())
            .equals_iterator(&[Modifier::Ctrl, Modifier::Alt].iter());

        assert_that!(binding.key).is_equal_to(xkb::KEY_A);

        assert_that!(binding.action).is_equal_to(Action::Run {
            env: vec![],
            command: "alacritty".to_string(),
        });
    }

    #[test]
    pub fn test() {
        let binding = vec![
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
            KeyBinding {
                modifiers: HashSet::from([Modifier::Alt]),
                key: xkb::KEY_a,
                action: Action::Close,
            },
            KeyBinding {
                modifiers: HashSet::from([Modifier::Alt]),
                key: xkb::KEY_v,
                action: Action::LayoutVertical,
            },
            KeyBinding {
                modifiers: HashSet::from([Modifier::Alt]),
                key: xkb::KEY_d,
                action: Action::LayoutHorizontal,
            },
            KeyBinding {
                modifiers: HashSet::from([Modifier::Ctrl, Modifier::Shift]),
                key: xkb::KEY_space,
                action: Action::ToggleFloating,
            },
            KeyBinding {
                modifiers: HashSet::from([Modifier::Alt]),
                key: xkb::KEY_k,
                action: Action::MoveFocusUp,
            },
            KeyBinding {
                modifiers: HashSet::from([Modifier::Alt]),
                key: xkb::KEY_h,
                action: Action::MoveFocusLeft,
            },
            KeyBinding {
                modifiers: HashSet::from([Modifier::Alt]),
                key: xkb::KEY_l,
                action: Action::MoveFocusRight,
            },
            KeyBinding {
                modifiers: HashSet::from([Modifier::Alt]),
                key: xkb::KEY_j,
                action: Action::MoveFocusDown,
            },
            KeyBinding {
                modifiers: HashSet::from([Modifier::Alt]),
                key: xkb::KEY_k,
                action: Action::MoveFocusUp,
            },
        ];

        let config = WazemmesConfig {
            gaps: 14,
            keybindings: binding,
        };

        let result = ron::to_string(&config).unwrap();
        println!("{:#}", result);
        println!("{:?}", ron::from_str::<WazemmesConfig>(&result).unwrap());
    }
}
