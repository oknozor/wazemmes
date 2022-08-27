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
use smithay::{delegate_primary_selection, delegate_seat};
use smithay::desktop::WindowSurfaceType;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point};
use smithay::input::keyboard::{keysyms as xkb, FilterResult};
use smithay::input::pointer::{CursorImageStatus, MotionEvent, PointerHandle};
use smithay::input::{Seat, SeatHandler};
use smithay::utils::SERIAL_COUNTER;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::data_device::set_data_device_focus;
use smithay::wayland::primary_selection::{PrimarySelectionHandler, PrimarySelectionState, set_primary_focus};
use smithay::reexports::wayland_server::Resource;

pub(crate) mod grabs;
pub mod handlers;

#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
    ToggleFullScreenWindow,
    ToggleFullScreenContainer,
    MoveWindow(Direction),
    MoveContainer(Direction),
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
                pointer.axis(&mut self.state, frame);
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
        let action = self.keyboard_key_to_action::<I>( event);
        if action != KeyAction::None {
            debug!("keyboard action triggered: {:?}", action)
        };

        match action {
            KeyAction::Run(cmd, env) => Self::run(cmd, env),
            KeyAction::Close => self.close(display),
            KeyAction::LayoutVertical => self.set_layout_v(),
            KeyAction::LayoutHorizontal => self.set_layout_h(),
            KeyAction::MoveToWorkspace(num) => self.state.move_to_workspace(num, display),
            KeyAction::MoveFocus(direction) => self.move_focus(direction, display),
            KeyAction::MoveWindow(direction) => self.move_window(direction, display),
            KeyAction::MoveContainer(direction) => self.move_container(direction),
            KeyAction::ToggleFloating => self.toggle_floating(),
            KeyAction::ToggleFullScreenWindow => self.toggle_fullscreen_window(),
            KeyAction::ToggleFullScreenContainer => self.toggle_fullscreen_container(),
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
            KeyAction::None => {}
        }
    }

    pub fn keyboard_key_to_action<I: InputBackend>(
        &mut self,
        evt: I::KeyboardKeyEvent,
    ) -> KeyAction {
        let keycode = evt.key_code();
        let state = evt.state();
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time(&evt);
        let keyboard = self.state.seat.get_keyboard().unwrap();
        let _mod_pressed = &self.state.mod_pressed;
        let bindings = self.state.config.keybindings.clone();

        let action = keyboard
            .input(&mut self.state, keycode, state, serial, time, |modifiers, handle| {
                let keysym = handle.modified_sym();

                if state == KeyState::Pressed {
                    let action: Option<FilterResult<KeyAction>> = self.state.config.keybindings
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
            .unwrap_or(KeyAction::None);

        debug!("Action triggered {:?}", action);
        action
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
            under,
            &MotionEvent {
                location: position,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
    }
}

// TODO : Move to handlers module
impl SeatHandler for Wazemmes {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;

    fn seat_state(&mut self) -> &mut smithay::input::SeatState<Self> {&mut self.seat_state}

    fn focus_changed(&mut self, seat: &Seat<Self>, surface: Option<&WlSurface>) {
        let dh = &self.display;

        let focus = surface.and_then(|s| dh.get_client(s.id()).ok());
        let focus2 = surface.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, focus);
        set_primary_focus(dh, seat, focus2);
    }

    fn cursor_image(&mut self, seat: &Seat<Self>, image: CursorImageStatus) {
        self.pointer_icon.on_new_cursor(image);
    }
}

delegate_seat!(Wazemmes);

impl PrimarySelectionHandler for Wazemmes {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

delegate_primary_selection!(Wazemmes);

