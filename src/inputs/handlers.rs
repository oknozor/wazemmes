use crate::inputs::grabs::MoveSurfaceGrab;
use crate::shell::container::{ContainerLayout, ContainerState};
use crate::shell::node::Node;
use crate::shell::window::{WindowState, WindowWrap, FLOATING_Z_INDEX};
use crate::state::CallLoopData;

use slog_scope::debug;
use smithay::backend::input::{
    Axis, Event, InputBackend, MouseButton, PointerAxisEvent, PointerButtonEvent,
};
use smithay::desktop::Window;
use smithay::nix::libc;
use smithay::reexports::wayland_server::protocol::wl_pointer;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point};
use smithay::wayland::seat::{AxisFrame, ButtonEvent, Focus};
use smithay::wayland::{Serial, SERIAL_COUNTER};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

impl CallLoopData {
    pub fn run(cmd: String) {
        let mut command = Command::new(cmd);
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
            container.close_window();
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
                handle.set_focus(display, Some(window.toplevel().wl_surface()), serial);
            }
        }

        let root = workspace.root();
        let mut root = root.get_mut();
        if !root.has_container() {
            let output = self.state.space.outputs().next().unwrap();
            let geo = self.state.space.output_geometry(output).unwrap();
            root.height = geo.size.h;
            root.width = geo.size.w;
        }

        let space = &mut self.state.space;
        root.redraw(space);
    }

    pub fn handle_pointer_axis<I: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        event: <I as InputBackend>::PointerAxisEvent,
    ) {
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

        self.state
            .seat
            .get_pointer()
            .unwrap()
            .axis(&mut self.state, dh, frame);
    }

    pub fn handle_pointer_button<I: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        event: &<I as InputBackend>::PointerButtonEvent,
    ) {
        let pointer = self.state.seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let button_state = wl_pointer::ButtonState::from(event.state());

        pointer.button(
            &mut self.state,
            dh,
            &ButtonEvent {
                button,
                state: button_state,
                serial,
                time: event.time(),
            },
        );

        if let Some(MouseButton::Left) = event.button() {
            if wl_pointer::ButtonState::Pressed == button_state {
                if let Some(window) = self
                    .state
                    .space
                    .window_under(pointer.current_location())
                    .cloned()
                {
                    let window = WindowWrap::from(window);

                    if self.state.mod_pressed && window.is_floating() {
                        let pos = pointer.current_location();
                        let initial_window_location = (pos.x as i32, pos.y as i32).into();
                        let start_data = pointer.grab_start_data().unwrap();

                        let window = window.get().clone();

                        let grab = MoveSurfaceGrab {
                            start_data,
                            window,
                            initial_window_location,
                        };

                        pointer.set_grab(grab, serial, Focus::Clear);
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
                        window.configure();
                    });

                    let keyboard = self.state.seat.get_keyboard().unwrap();
                    keyboard.set_focus(dh, None, serial);
                }
            }
        }
    }

    fn toggle_window_focus(&mut self, dh: &DisplayHandle, serial: Serial, window: &Window) {
        let keyboard = self.state.seat.get_keyboard().unwrap();

        self.state.space.windows().for_each(|window| {
            window.set_activated(false);
            window.configure();
        });

        let window = WindowWrap::from(window.clone());
        let location = self.state.space.window_bbox(window.get()).unwrap().loc;

        self.state
            .space
            .map_window(window.get(), location, window.z_index(), true);
        keyboard.set_focus(dh, Some(window.toplevel().wl_surface()), serial);
        window.get().set_activated(true);
        window.get().configure();
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

    pub fn toggle_floating(&mut self) {
        let ws = self.state.get_current_workspace();
        let ws = ws.get();
        let focus = ws.get_focus();

        if let Some(window) = focus.1 {
            let output = self.state.space.outputs().next().unwrap();
            let geometry = self.state.space.output_geometry(output).unwrap();
            let y = geometry.size.h / 2 + geometry.loc.y;
            let x = geometry.size.w / 2 + geometry.loc.x;

            window.toggle_floating();
            self.state
                .space
                .map_window(window.get(), (x, y), FLOATING_Z_INDEX, true);
            window.get().configure();
            let space = &mut self.state.space;
            focus.0.get_mut().redraw(space);
        }
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

#[derive(Debug, PartialEq, Eq)]
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
