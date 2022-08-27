use crate::inputs::grabs::MoveSurfaceGrab;
use crate::shell::container::{ContainerLayout, ContainerState};
use crate::shell::node::Node;
use crate::shell::window::{WindowState, WindowWrap};
use crate::state::CallLoopData;

use slog_scope::{debug, warn};
use smithay::backend::input::{Axis, AxisSource, ButtonState, Event, InputBackend, MouseButton, PointerAxisEvent, PointerButtonEvent};
use smithay::desktop::{Kind, Window};
use smithay::nix::libc;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point};
use smithay::input::pointer::{AxisFrame, ButtonEvent, Focus};
use smithay::utils::{Serial, SERIAL_COUNTER};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

impl CallLoopData {
    pub fn run(cmd: String, env: impl IntoIterator<Item = (String, String)>) {
        let mut command = Command::new(cmd);

        for (var, value) in env {
            command.env(var, value);
        }

        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        // Setup double-fork to avoid zombies.
        unsafe {
            command.pre_exec(|| {
                match libc::fork() {
                    -1 => return Err(io::Error::last_os_error()),
                    0 => (),
                    _ => libc::_exit(0),
                }

                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }

                Ok(())
            });
        }

        command.spawn().unwrap().wait().unwrap();
    }

    pub fn close(&mut self, display: &DisplayHandle) {
        let state = {
            let container = self.state.get_current_workspace().get_mut().get_focus().0;

            let mut container = container.get_mut();
            debug!("Closing window in container: {}", container.id);
            container.close_window(self.state.x11_state.as_mut());
            container.state()
        };

        match state {
            ContainerState::Empty => {
                debug!("Closing empty container");
                let ws = self.state.get_current_workspace();
                let mut ws = ws.get_mut();
                ws.pop_container();
                if let Some(window) = ws.get_focus().1 {
                    self.toggle_window_focus(display, SERIAL_COUNTER.next_serial(), window.get());
                }
            }
            ContainerState::HasContainersOnly => {
                debug!("Draining window from container");
                {
                    let container = {
                        let ws = self.state.get_current_workspace();
                        let ws = &ws.get_mut();
                        ws.get_focus().0
                    };

                    let children: Option<Vec<(u32, Node)>> = {
                        let mut container = container.get_mut();
                        if container.parent.is_some() {
                            Some(container.nodes.drain_all())
                        } else {
                            None
                        }
                    };

                    let mut container = container.get_mut();
                    let id = container.id;

                    if let (Some(parent), Some(children)) = (&mut container.parent, children) {
                        let mut parent = parent.get_mut();
                        parent.nodes.remove(&id);
                        parent.nodes.extend(children);
                    }
                }

                let ws = self.state.get_current_workspace();
                let ws = ws.get();

                if let Some(window) = ws.get_focus().1 {
                    self.toggle_window_focus(display, SERIAL_COUNTER.next_serial(), window.get());
                }
            }
            ContainerState::HasWindows => {
                let ws = self.state.get_current_workspace();
                let ws = ws.get();

                if let Some(window) = ws.get_focus().1 {
                    self.toggle_window_focus(display, SERIAL_COUNTER.next_serial(), window.get());
                }
                debug!("Cannot remove non empty container");
            }
        };

        // Reset focus
        let workspace = self.state.get_current_workspace();
        let workspace = workspace.get();

        {
            if let Some(window) = workspace.get_focus().1 {
                let handle = self
                    .state
                    .seat
                    .get_keyboard()
                    .expect("Should have a keyboard seat");

                let serial = SERIAL_COUNTER.next_serial();
                handle.set_focus(&mut self.state, Some(window.wl_surface()), serial);
            }
        }

        workspace.redraw(
            &mut self.state.space,
            display,
            self.state.x11_state.as_mut(),
        );
    }

    pub fn handle_pointer_button<I: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        event: &<I as InputBackend>::PointerButtonEvent,
    ) {
        let pointer = self.state.seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let button_state = ButtonState::from(event.state());

        pointer.button(
            &mut self.state,
            &ButtonEvent {
                button,
                state: button_state,
                serial,
                time: event.time(),
            },
        );

        if let Some(MouseButton::Left) = event.button() {
            if ButtonState::Pressed == button_state {
                if let Some(window) = self
                    .state
                    .space
                    .window_under(pointer.current_location())
                    .cloned()
                {
                    let window = WindowWrap::from(window);

                    if *self.state.mod_pressed.borrow() && window.is_floating() {
                        let pos = pointer.current_location();
                        let initial_window_location = (pos.x as i32, pos.y as i32).into();
                        let start_data = pointer.grab_start_data().unwrap();

                        let window = window.get().clone();

                        let grab = MoveSurfaceGrab {
                            start_data,
                            window,
                            initial_window_location,
                        };

                        pointer.set_grab(&mut self.state, grab, serial, Focus::Clear);
                    } else {
                        let id = window.id();
                        let ws = self.state.get_current_workspace();
                        let mut ws = ws.get_mut();
                        let container = ws.root().container_having_window(id).unwrap();
                        container.get_mut().set_focus(id);
                        self.toggle_window_focus(dh, serial, window.get());
                        let focused_id = container.get().id;
                        ws.set_container_focused(focused_id);
                    }
                } else {
                    self.state.space.windows().for_each(|window| {
                        window.set_activated(false);
                        match window.toplevel() {
                            Kind::Xdg(_) => window.configure(),
                            Kind::X11(_) => {
                                warn!("Skip window configure for X11 surface")
                            }
                        }
                    });

                    let keyboard = self.state.seat.get_keyboard().unwrap();
                    keyboard.set_focus(&mut self.state, None, serial);
                }
            }
        }
    }

    fn toggle_window_focus(&mut self, dh: &DisplayHandle, serial: Serial, window: &Window) {
        let keyboard = self.state.seat.get_keyboard().unwrap();

        self.state.space.windows().for_each(|window| {
            window.set_activated(false);
            match window.toplevel() {
                Kind::Xdg(_) => window.configure(),
                Kind::X11(_) => warn!("Skip window configure for X11 surface"),
            }
        });

        let window = WindowWrap::from(window.clone());
        let location = self.state.space.window_bbox(window.get()).unwrap().loc;

        self.state
            .space
            .map_window(window.get(), location, window.z_index(), true);

        keyboard.set_focus(&mut self.state, Some(window.wl_surface()), serial);

        let window = window.get();
        window.set_activated(true);
        match window.toplevel() {
            Kind::Xdg(_) => window.configure(),
            Kind::X11(_) => {
                // cnoop
            }
        }
    }

    pub fn set_layout_h(&mut self) {
        self.state.next_layout = Some(ContainerLayout::Horizontal)
    }

    pub fn set_layout_v(&mut self) {
        self.state.next_layout = Some(ContainerLayout::Vertical)
    }

    pub fn move_focus(&mut self, direction: Direction, display: &DisplayHandle) {
        let window = self.scan_window(direction);

        if let Some(window) = window {
            let serial = SERIAL_COUNTER.next_serial();
            let id = window.user_data().get::<WindowState>().unwrap().id();
            let ws = self.state.get_current_workspace();
            let mut ws = ws.get_mut();
            let container = ws.root().container_having_window(id).unwrap();
            container
                .get_mut()
                .set_focus(WindowWrap::from(window.clone()).id());
            let focused_id = container.get().id;
            ws.set_container_focused(focused_id);
            self.toggle_window_focus(display, serial, &window);
        }
    }

    pub fn move_window(&mut self, direction: Direction, dh: &DisplayHandle) {
        // TODO: this should be simplified !
        let new_focus = {
            let ws = self.state.get_current_workspace();
            let ws = ws.get();
            let (container, window) = ws.get_focus();

            match window {
                Some(window) => {
                    let target = self
                        .scan_window(direction)
                        .map(|target| target.user_data().get::<WindowState>().unwrap().id())
                        .and_then(|id| {
                            ws.root()
                                .container_having_window(id)
                                .map(|container| (id, container))
                        });

                    if let Some((target_window_id, target_container)) = target {
                        let target_container_id = target_container.get().id;
                        let current_container_id = container.get().id;

                        // Ensure we are not taking a double borrow if window moves in the same container
                        if target_container_id == current_container_id {
                            let mut container = container.get_mut();
                            container.nodes.remove(&window.id());
                            match direction {
                                Direction::Left | Direction::Up => {
                                    container.insert_window_before(target_window_id, window)
                                }
                                Direction::Right | Direction::Down => {
                                    container.insert_window_after(target_window_id, window)
                                }
                            }
                        } else {
                            let container_state = {
                                let mut target_container = target_container.get_mut();
                                let mut current = container.get_mut();
                                current.nodes.remove(&window.id());
                                match direction {
                                    Direction::Left | Direction::Up => target_container
                                        .insert_window_after(target_window_id, window),
                                    Direction::Right | Direction::Down => target_container
                                        .insert_window_before(target_window_id, window),
                                }

                                current.state()
                            };

                            if container_state == ContainerState::Empty {
                                let container = container.get();
                                if let Some(parent) = &container.parent {
                                    let mut parent = parent.get_mut();
                                    parent.nodes.remove(&container.id);
                                }
                            }
                        }

                        Some(target_container_id)
                    } else {
                        None
                    }
                }
                None => None,
            }
        };

        if let Some(new_focus) = new_focus {
            let ws = self.state.get_current_workspace();
            let mut ws = ws.get_mut();
            ws.set_container_focused(new_focus);
        }

        let ws = self.state.get_current_workspace();
        let ws = ws.get();
        ws.redraw(&mut self.state.space, dh, self.state.x11_state.as_mut());
    }

    pub fn move_container(&self, _direction: Direction) {
        todo!("move container not implemented")
    }

    pub fn toggle_floating(&mut self) {
        let ws = self.state.get_current_workspace();
        let ws = ws.get();
        let focus = ws.get_focus();

        if let Some(window) = focus.1 {
            window.toggle_floating();
            let space = &mut self.state.space;
            let x11_state = self.state.x11_state.as_mut();
            focus.0.get_mut().redraw(space, x11_state);
        }
    }

    pub fn toggle_fullscreen_window(&mut self) {
        let ws = self.state.get_current_workspace();
        let mut ws = ws.get_mut();
        if ws.fullscreen_layer.is_some() {
            ws.fullscreen_layer = None
        } else {
            let (_c, window) = ws.get_focus();
            if let Some(window) = window {
                ws.fullscreen_layer = Some(Node::Window(window));
            }
        }

        ws.redraw(
            &mut self.state.space,
            &self.state.display,
            self.state.x11_state.as_mut(),
        );
    }

    pub fn toggle_fullscreen_container(&mut self) {
        let ws = self.state.get_current_workspace();
        let mut ws = ws.get_mut();
        if ws.fullscreen_layer.is_some() {
            ws.reset_gaps(&self.state.space);
            ws.fullscreen_layer = None
        } else {
            let (container, _) = ws.get_focus();
            ws.unmap_all(&mut self.state.space);
            container
                .get_mut()
                .toggle_fullscreen(&mut self.state.space, self.state.x11_state.as_mut());
            ws.fullscreen_layer = Some(Node::Container(container));
        }

        ws.redraw(
            &mut self.state.space,
            &self.state.display,
            self.state.x11_state.as_mut(),
        );
    }

    fn scan_window(&mut self, direction: Direction) -> Option<Window> {
        let ws = self.state.get_current_workspace();
        let ws = ws.get();
        let focus = ws.get_focus();
        let mut window = None;

        if let Some(window_ref) = focus.1 {
            let loc = self
                .state
                .space
                .window_location(window_ref.get())
                .expect("window should have a location");

            let (mut x, mut y) = (loc.x, loc.y);
            let width = window_ref.get().geometry().size.w;
            let height = window_ref.get().geometry().size.h;

            // Move one pixel inside the window to avoid being out of bbox after converting to f64
            match direction {
                Direction::Right => {
                    x += width;
                    y += 1;
                }
                Direction::Down => y += height - 1,
                Direction::Left => y += 1,
                Direction::Up => x += 1,
            }

            let mut point = Point::from((x, y)).to_f64();
            while window.is_none() {
                if self.state.space.output_under(point).next().is_none() {
                    break;
                }

                direction.update_point(&mut point);

                window = {
                    let window = self.state.space.window_under(point);
                    window.cloned()
                };
            }
        }
        window
    }
}

pub fn basic_axis_frame<I: InputBackend>(evt: &I::PointerAxisEvent) -> AxisFrame {
    let source = match evt.source() {
        AxisSource::Continuous => AxisSource::Continuous,
        AxisSource::Finger => AxisSource::Finger,
        AxisSource::Wheel | AxisSource::WheelTilt => AxisSource::Wheel,
    };
    let horizontal_amount = evt
        .amount(Axis::Horizontal)
        .unwrap_or_else(|| evt.amount_discrete(Axis::Horizontal).unwrap_or(0.0) * 3.0);
    let vertical_amount = evt
        .amount(Axis::Vertical)
        .unwrap_or_else(|| evt.amount_discrete(Axis::Vertical).unwrap_or(0.0) * 3.0);
    let horizontal_amount_discrete = evt.amount_discrete(Axis::Horizontal);
    let vertical_amount_discrete = evt.amount_discrete(Axis::Vertical);

    let mut frame = AxisFrame::new(evt.time()).source(source);
    if horizontal_amount != 0.0 {
        frame = frame.value(Axis::Horizontal, horizontal_amount);
        if let Some(discrete) = horizontal_amount_discrete {
            frame = frame.discrete(Axis::Horizontal, discrete as i32);
        }
    } else if source == AxisSource::Finger {
        frame = frame.stop(Axis::Horizontal);
    }

    if vertical_amount != 0.0 {
        frame = frame.value(Axis::Vertical, vertical_amount);
        if let Some(discrete) = vertical_amount_discrete {
            frame = frame.discrete(Axis::Vertical, discrete as i32);
        }
    } else if source == AxisSource::Finger {
        frame = frame.stop(Axis::Vertical);
    }

    frame
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    fn update_point(&self, p: &mut Point<f64, Logical>) {
        match self {
            Direction::Left => p.x -= 1.0,
            Direction::Right => p.x += 1.0,
            Direction::Up => p.y -= 1.0,
            Direction::Down => p.y += 1.0,
        }
    }
}
