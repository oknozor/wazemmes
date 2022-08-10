use crate::shell::container::{ContainerLayout, ContainerState};
use slog::debug;
use smithay::backend::input::KeyState;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::{
    backend::input::{
        Axis, Event, InputBackend, InputEvent, KeyboardKeyEvent, PointerAxisEvent,
        PointerButtonEvent, PointerMotionAbsoluteEvent,
    },
    reexports::wayland_server::{protocol::wl_pointer, Display},
    wayland::{
        seat::{AxisFrame, ButtonEvent, FilterResult, MotionEvent},
        SERIAL_COUNTER,
    },
};

use smithay::wayland::seat::keysyms as xkb;
use crate::shell::tree::ContainerRef;
use crate::state::Wazemmes;

#[derive(Debug, PartialEq)]
enum KeyAction {
    Run(String),
    MoveToWorkspace(u8),
    LayoutVertical,
    LayoutHorizontal,
    Close,
    None,
}

impl Wazemmes {
    pub fn process_input_event<I: InputBackend>(
        &mut self,
        display: &mut Display<Wazemmes>,
        event: InputEvent<I>,
    ) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let action = self.keyboard_key_to_action::<I>(&display.handle(), event);
                if action != KeyAction::None {
                    debug!(&self.log, "keyboard action triggered: {:?}", action)
                };

                match action {
                    KeyAction::Run(cmd) => {
                        std::process::Command::new(cmd).spawn().ok();
                    }
                    KeyAction::Close => {
                        let state = {
                            let container = self.get_current_workspace()
                                .get_mut()
                                .tree.get_container_focused();
                            let mut container = container.borrow_mut();
                            debug!(&self.log, "Closing window in container: {}", container.id);
                            container.close_window();
                            container.state()
                        };

                        match state {
                            ContainerState::Empty => {
                                println!("empty container removed");
                                self.get_current_workspace()
                                    .get_mut()
                                    .tree
                                    .pop();
                            }
                            ContainerState::HasChildrenOnly => {
                                let container = self.get_current_workspace()
                                    .get_mut()
                                    .tree
                                    .get_container_focused();
                                let copy = container.clone();
                                let mut container = container.borrow_mut();
                                let id = container.id;
                                if let Some(parent) = &mut container.parent {
                                    let childs: Vec<ContainerRef> = copy.borrow_mut().childs.drain(..).collect();
                                    let mut parent = parent.borrow_mut();
                                    parent.childs.extend_from_slice(childs.as_slice());
                                    let parent_id = parent.id;
                                    println!("Container [{id}], was removed, child container where reassigned to container [{parent_id}]");
                                }
                            }
                            ContainerState::HasWindows => {
                                println!("Cannot remove non empty container");
                            }
                        };

                        // Reset focus
                        let container = self.get_current_workspace()
                            .get_mut()
                            .tree
                            .get_container_focused();
                        let container = container.borrow_mut();
                        container.redraw(&mut self.space);
                        let window = container.windows.last().unwrap();
                        let handle = self
                            .seat
                            .get_keyboard()
                            .expect("Should have a keyboard seat");

                        let serial = SERIAL_COUNTER.next_serial();
                        handle.set_focus(&display.handle(), Some(window.get_toplevel().wl_surface()), serial);
                    }
                    KeyAction::LayoutVertical => self.next_layout = Some(ContainerLayout::Vertical),
                    KeyAction::LayoutHorizontal => self.next_layout = Some(ContainerLayout::Horizontal),
                    KeyAction::None => {}
                    KeyAction::MoveToWorkspace(num) => self.move_to_workspace(num, &display.handle()),
                }
            }
            InputEvent::PointerMotion { .. } => {}
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output = self.space.outputs().next().unwrap();

                let output_geo = self.space.output_geometry(output).unwrap();

                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

                let serial = SERIAL_COUNTER.next_serial();

                let pointer = self.seat.get_pointer().unwrap();

                let under = self.surface_under_pointer(&pointer);

                let dh = &mut display.handle();
                pointer.motion(
                    self,
                    dh,
                    &MotionEvent {
                        location: pos,
                        focus: under,
                        serial,
                        time: event.time(),
                    },
                );
            }
            InputEvent::PointerButton { event, .. } => {
                let dh = &mut display.handle();
                let pointer = self.seat.get_pointer().unwrap();
                let keyboard = self.seat.get_keyboard().unwrap();
                let serial = SERIAL_COUNTER.next_serial();

                let button = event.button_code();

                let button_state = wl_pointer::ButtonState::from(event.state());

                if wl_pointer::ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                    if let Some(window) =
                    self.space.window_under(pointer.current_location()).cloned()
                    {
                        self.space.windows().for_each(|window| {
                            window.set_activated(false);
                            window.configure();
                        });

                        self.space.raise_window(&window, true);
                        keyboard.set_focus(dh, Some(window.toplevel().wl_surface()), serial);
                        window.set_activated(true);
                        window.configure();
                    } else {
                        self.space.windows().for_each(|window| {
                            window.set_activated(false);
                            window.configure();
                        });
                        keyboard.set_focus(dh, None, serial);
                    }
                };

                pointer.button(
                    self,
                    dh,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time(),
                    },
                );
            }
            InputEvent::PointerAxis { event, .. } => {
                let source = wl_pointer::AxisSource::from(event.source());

                let horizontal_amount = event
                    .amount(Axis::Horizontal)
                    .unwrap_or_else(|| event.amount_discrete(Axis::Horizontal).unwrap() * 3.0);
                let vertical_amount = event
                    .amount(Axis::Vertical)
                    .unwrap_or_else(|| event.amount_discrete(Axis::Vertical).unwrap() * 3.0);
                let horizontal_amount_discrete = event.amount_discrete(Axis::Horizontal);
                let vertical_amount_discrete = event.amount_discrete(Axis::Vertical);

                let mut frame = AxisFrame::new(event.time()).source(source);
                if horizontal_amount != 0.0 {
                    frame = frame.value(wl_pointer::Axis::HorizontalScroll, horizontal_amount);
                    if let Some(discrete) = horizontal_amount_discrete {
                        frame = frame.discrete(wl_pointer::Axis::HorizontalScroll, discrete as i32);
                    }
                } else if source == wl_pointer::AxisSource::Finger {
                    frame = frame.stop(wl_pointer::Axis::HorizontalScroll);
                }
                if vertical_amount != 0.0 {
                    frame = frame.value(wl_pointer::Axis::VerticalScroll, vertical_amount);
                    if let Some(discrete) = vertical_amount_discrete {
                        frame = frame.discrete(wl_pointer::Axis::VerticalScroll, discrete as i32);
                    }
                } else if source == wl_pointer::AxisSource::Finger {
                    frame = frame.stop(wl_pointer::Axis::VerticalScroll);
                }

                let dh = &mut display.handle();
                self.seat.get_pointer().unwrap().axis(self, dh, frame);
            }
            _ => {}
        }
    }

    fn keyboard_key_to_action<B: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        evt: B::KeyboardKeyEvent,
    ) -> KeyAction {
        let keycode = evt.key_code();
        let state = evt.state();
        debug!(self.log, "key"; "keycode" => keycode, "state" => format!("{:?}", state));
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time(&evt);
        let keyboard = self.seat.get_keyboard().unwrap();

        keyboard
            .input(dh, keycode, state, serial, time, |modifiers, handle| {
                let keysyms = handle.modified_syms();

                // todo: log level println!("Keystroke: (Mod={}, keysym={:?})", modifiers.logo, keysyms);

                if modifiers.alt && keysyms.contains(&xkb::KEY_t) && state == KeyState::Pressed {
                    FilterResult::Intercept(KeyAction::Run("alacritty".to_string()))
                } else if modifiers.alt && keysyms.contains(&xkb::KEY_q) && state == KeyState::Pressed {
                    FilterResult::Intercept(KeyAction::Close)
                } else if modifiers.alt && keysyms.contains(&xkb::KEY_d) && state == KeyState::Pressed {
                    FilterResult::Intercept(KeyAction::LayoutHorizontal)
                } else if modifiers.alt && keysyms.contains(&xkb::KEY_v) && state == KeyState::Pressed {
                    FilterResult::Intercept(KeyAction::LayoutVertical)
                } else if modifiers.alt && keysyms.contains(&xkb::KEY_ampersand) && state == KeyState::Pressed {
                    FilterResult::Intercept(KeyAction::MoveToWorkspace(0))
                } else if modifiers.alt && keysyms.contains(&xkb::KEY_eacute) && state == KeyState::Pressed {
                    FilterResult::Intercept(KeyAction::MoveToWorkspace(1))
                } else {
                    FilterResult::Forward
                }
            })
            .unwrap_or(KeyAction::None)
    }
}
