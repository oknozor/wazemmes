use crate::backend::{BackendHandler, InputHandler, OutputId};
use crate::config::keybinding::Action;
use crate::inputs::handlers::Direction;
use crate::state::seat::SeatState;
use crate::{CallLoopData, Wazemmes};
use slog_scope::{debug, info};
use smithay::backend::input::{
    AbsolutePositionEvent, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
    PointerMotionEvent,
};
use smithay::backend::session::auto::AutoSession;
use smithay::backend::session::Session;
use smithay::desktop::WindowSurfaceType;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point};
use smithay::wayland::seat::{keysyms as xkb, FilterResult, MotionEvent, PointerHandle};
use smithay::wayland::SERIAL_COUNTER;

pub(crate) mod grabs;
pub mod handlers;

#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
    MoveFocus(Direction),
    Run(String, Vec<(String, String)>),
    MoveToWorkspace(u8),
    LayoutVertical,
    LayoutHorizontal,
    ToggleFloating,
    VtSwitch(i32),
    Close,
    Quit,
    None,
}

impl InputHandler for CallLoopData {
    fn process_input_event<I: InputBackend>(
        &mut self,
        event: InputEvent<I>,
        output_id: Option<&OutputId>,
        session: Option<&mut AutoSession>,
    ) {
        let absolute_output = self
            .state
            .space
            .outputs()
            .find(|o| o.user_data().get::<OutputId>() == output_id)
            .cloned();

        match event {
            InputEvent::Keyboard { event, .. } => {
                self.process_shortcut::<I>(&self.display.handle(), event, session)
            }
            InputEvent::PointerMotion { event } => {
                let pointer = self.state.seat.get_pointer().unwrap();
                let seat_state = SeatState::for_seat(&self.state.seat);

                let mut position = seat_state.pointer_pos() + event.delta();

                let max_x = self.state.space.outputs().fold(0, |acc, o| {
                    acc + self.state.space.output_geometry(o).unwrap().size.w
                });

                let max_y = self
                    .state
                    .space
                    .outputs()
                    .next()
                    .map(|o| self.state.space.output_geometry(o).unwrap().size.h)
                    .unwrap_or_default();

                position.x = position.x.max(0.0).min(max_x as f64 - 1.0);
                position.y = position.y.max(0.0).min(max_y as f64 - 1.0);

                seat_state.set_pointer_pos(position);
                self.state.pointer_motion(pointer, position, event.time());
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let pointer = self.state.seat.get_pointer().unwrap();

                let output = absolute_output
                    .unwrap_or_else(|| self.state.space.outputs().next().unwrap().clone());
                let output_geo = self.state.space.output_geometry(&output).unwrap();
                let output_loc = output_geo.loc.to_f64();

                let position = output_loc + event.position_transformed(output_geo.size);

                SeatState::for_seat(&self.state.seat).set_pointer_pos(position);
                self.state.pointer_motion(pointer, position, event.time());
            }
            InputEvent::PointerButton { event, .. } => {
                self.handle_pointer_button::<I>(&self.display.handle(), &event)
            }
            InputEvent::PointerAxis { event, .. } => {
                let frame = handlers::basic_axis_frame::<I>(&event);

                let pointer = self.state.seat.get_pointer().unwrap();
                pointer.axis(&mut self.state, &self.display.handle(), frame);
            }
            _ => {}
        }
    }
}

impl CallLoopData {
    fn process_shortcut<I: InputBackend>(
        &mut self,
        display: &DisplayHandle,
        event: <I as InputBackend>::KeyboardKeyEvent,
        session: Option<&mut AutoSession>,
    ) {
        let action = self.keyboard_key_to_action::<I>(display, event);
        if action != KeyAction::None {
            debug!("keyboard action triggered: {:?}", action)
        };

        match action {
            KeyAction::Run(cmd, env) => Self::run(cmd, env),
            KeyAction::Close => self.close(display),
            KeyAction::LayoutVertical => self.set_layout_v(),
            KeyAction::LayoutHorizontal => self.set_layout_h(),
            KeyAction::None => {}
            KeyAction::MoveToWorkspace(num) => self.state.move_to_workspace(num, display),
            KeyAction::MoveFocus(direction) => self.move_focus(direction, display),
            KeyAction::ToggleFloating => self.toggle_floating(),
            KeyAction::Quit => {
                info!("Quitting");
                self.close_compositor();
            }
            KeyAction::VtSwitch(vt) => {
                if let Some(session) = session {
                    session.change_vt(vt as i32).ok();
                } else {
                    debug!("VtSwitch is not supported with this backend")
                }
            }
        }
    }

    pub fn keyboard_key_to_action<I: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        evt: I::KeyboardKeyEvent,
    ) -> KeyAction {
        let keycode = evt.key_code();
        let state = evt.state();
        // TODO: can we filter that with env var ?
        // debug!("key"; "keycode" => keycode, "state" => format!("{:?}", state));
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time(&evt);
        let keyboard = self.state.seat.get_keyboard().unwrap();

        keyboard
            .input(dh, keycode, state, serial, time, |modifiers, handle| {
                if modifiers.alt {
                    self.state.mod_pressed = true
                } else {
                    self.state.mod_pressed = false
                };

                let keysym = handle.modified_sym();

                if state == KeyState::Pressed {
                    let bindings = &self.state.config.keybindings;
                    let action: Option<FilterResult<KeyAction>> = bindings
                        .iter()
                        .find_map(|binding| binding.match_action(*modifiers, keysym))
                        .map(Action::into)
                        .map(FilterResult::Intercept);

                    match action {
                        None => match keysym {
                            xkb::KEY_XF86Switch_VT_1..=xkb::KEY_XF86Switch_VT_12 => {
                                FilterResult::Intercept(KeyAction::VtSwitch(
                                    (keysym - xkb::KEY_XF86Switch_VT_1 + 1) as i32,
                                ))
                            }
                            _ => FilterResult::Forward,
                        },
                        Some(action) => action,
                    }
                } else {
                    FilterResult::Forward
                }
            })
            .unwrap_or(KeyAction::None)
    }
}

impl Wazemmes {
    fn pointer_motion(
        &mut self,
        pointer: PointerHandle<Self>,
        position: Point<f64, Logical>,
        time: u32,
    ) {
        let under = self
            .space
            .surface_under(position, WindowSurfaceType::all())
            .map(|(_, surface, location)| (surface, location));

        let dh = self.display.clone();
        pointer.motion(
            self,
            &dh,
            &MotionEvent {
                location: position,
                focus: under,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
    }
}
