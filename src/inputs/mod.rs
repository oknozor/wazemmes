use slog_scope::debug;
use smithay::backend::input::{Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent};

use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::wayland::seat::FilterResult;
use smithay::wayland::SERIAL_COUNTER;

use crate::state::Wazemmes;
use crate::Backend;
use handlers::Direction;
use smithay::wayland::seat::keysyms as xkb;

pub(crate) mod grabs;
mod handlers;

#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
    MoveFocus(Direction),
    Run(String),
    MoveToWorkspace(u8),
    LayoutVertical,
    LayoutHorizontal,
    ToggleFloating,
    Close,
    None,
}

impl<B: Backend> Wazemmes<B> {
    pub fn process_input_event<I: InputBackend>(
        &mut self,
        display: &mut Display<Wazemmes<B>>,
        event: InputEvent<I>,
    ) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let action = self.keyboard_key_to_action::<I>(&display.handle(), event);
                if action != KeyAction::None {
                    debug!("keyboard action triggered: {:?}", action)
                };

                match action {
                    KeyAction::Run(cmd) => Self::run(cmd),
                    KeyAction::Close => self.close(&display),
                    KeyAction::LayoutVertical => self.set_layout_v(),
                    KeyAction::LayoutHorizontal => self.set_layout_h(),
                    KeyAction::None => {}
                    KeyAction::MoveToWorkspace(num) => {
                        self.move_to_workspace(num, &display.handle())
                    }
                    KeyAction::MoveFocus(direction) => self.move_focus(direction, display),
                    KeyAction::ToggleFloating => self.toggle_floating(),
                }
            }
            InputEvent::PointerMotion { .. } => {}
            InputEvent::PointerMotionAbsolute { event, .. } => {
                self.handle_pointer_motion::<I>(&display, &event)
            }
            InputEvent::PointerButton { event, .. } => {
                self.handle_pointer_button::<I>(&display, &event)
            }
            InputEvent::PointerAxis { event, .. } => self.handle_pointer_axis::<I>(&display, event),
            _ => {}
        }
    }

    pub fn keyboard_key_to_action<I: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        evt: I::KeyboardKeyEvent,
    ) -> KeyAction {
        let keycode = evt.key_code();
        let state = evt.state();
        debug!("key"; "keycode" => keycode, "state" => format!("{:?}", state));
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time(&evt);
        let keyboard = self.seat.get_keyboard().unwrap();

        keyboard
            .input(dh, keycode, state, serial, time, |modifiers, handle| {
                if modifiers.alt {
                    debug!("Mod active");
                    self.mod_pressed = true
                } else {
                    debug!("Mod released");
                    self.mod_pressed = false
                };

                let keysyms = handle.modified_syms();
                if modifiers.alt && state == KeyState::Pressed {
                    match keysyms {
                        [xkb::KEY_t] => {
                            FilterResult::Intercept(KeyAction::Run("alacritty".to_string()))
                        }
                        [xkb::KEY_q] => FilterResult::Intercept(KeyAction::Close),
                        [xkb::KEY_d] => FilterResult::Intercept(KeyAction::LayoutHorizontal),
                        [xkb::KEY_v] => FilterResult::Intercept(KeyAction::LayoutVertical),
                        [xkb::KEY_ampersand] => {
                            FilterResult::Intercept(KeyAction::MoveToWorkspace(0))
                        }
                        [xkb::KEY_eacute] => FilterResult::Intercept(KeyAction::MoveToWorkspace(1)),
                        _ => FilterResult::Forward,
                    }
                } else if modifiers.ctrl && state == KeyState::Pressed {
                    match keysyms {
                        [xkb::KEY_h] => {
                            FilterResult::Intercept(KeyAction::MoveFocus(Direction::Left))
                        }
                        [xkb::KEY_j] => {
                            FilterResult::Intercept(KeyAction::MoveFocus(Direction::Down))
                        }
                        [xkb::KEY_k] => {
                            FilterResult::Intercept(KeyAction::MoveFocus(Direction::Up))
                        }
                        [xkb::KEY_l] => {
                            FilterResult::Intercept(KeyAction::MoveFocus(Direction::Right))
                        }
                        _ => FilterResult::Forward,
                    }
                } else if modifiers.shift && modifiers.ctrl {
                    match keysyms {
                        [xkb::KEY_space] => FilterResult::Intercept(KeyAction::ToggleFloating),
                        _ => FilterResult::Forward,
                    }
                } else {
                    FilterResult::Forward
                }
            })
            .unwrap_or(KeyAction::None)
    }
}
