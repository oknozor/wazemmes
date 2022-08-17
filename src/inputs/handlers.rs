use crate::shell::container::{ContainerLayout, ContainerState};
use slog_scope::debug;
use smithay::backend::input::{
    Axis, Event, InputBackend, PointerAxisEvent, PointerButtonEvent, PointerMotionAbsoluteEvent,
};
use smithay::desktop::Window;
use smithay::reexports::wayland_server::protocol::wl_pointer;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::reexports::x11rb::protocol::xproto::ConnectionExt;
use smithay::utils::{Logical, Point};
use smithay::wayland::seat::{AxisFrame, ButtonEvent, Focus, MotionEvent};
use smithay::wayland::{Serial, SERIAL_COUNTER};

use crate::inputs::grabs::MoveSurfaceGrab;
use crate::shell::node::Node;
use crate::shell::window::{WindowState, WindowWarp, FLOATING_Z_INDEX};
use crate::state::Wazemmes;
use crate::Backend;

impl<B: Backend> Wazemmes<B> {
    pub fn run(cmd: String) {
        std::process::Command::new(cmd).spawn().ok();
    }

    pub fn close(&mut self, display: &&mut Display<Wazemmes<B>>) {
        let state = {
            let container = self
                .get_current_workspace()
                .get_mut()
                .get_container_focused();
            let mut container = container.get_mut();
            debug!("Closing window in container: {}", container.id);
            container.close_window();
            container.state()
        };

        match state {
            ContainerState::Empty => {
                debug!("Closing empty container");
                let ws = self.get_current_workspace();
                let mut ws = ws.get_mut();
                ws.pop_container();
                let focused = ws.get_container_focused();
                let focused = focused.get();
                let window = focused.get_focused_window();
                if let Some((_id, window)) = window {
                    self.set_window_focus(
                        &mut display.handle(),
                        SERIAL_COUNTER.next_serial(),
                        window.get(),
                    );
                }
            }
            ContainerState::HasContainersOnly => {
                debug!("Draining window from container");
                let container = {
                    let ws = self.get_current_workspace();
                    let ws = &ws.get_mut();
                    ws.get_container_focused()
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

                let ws = self.get_current_workspace();
                let ws = ws.get_mut();
                let focused = ws.get_container_focused();
                let focused = focused.get();
                let window = focused.get_focused_window();
                if let Some((_id, window)) = window {
                    self.set_window_focus(
                        &mut display.handle(),
                        SERIAL_COUNTER.next_serial(),
                        window.get(),
                    );
                }
            }
            ContainerState::HasWindows => {
                let ws = self.get_current_workspace();
                let ws = ws.get_mut();
                let focused = ws.get_container_focused();
                let focused = focused.get();
                let window = focused.get_focused_window();

                if let Some((_id, window)) = window {
                    self.set_window_focus(
                        &mut display.handle(),
                        SERIAL_COUNTER.next_serial(),
                        window.get(),
                    );
                }
                debug!("Cannot remove non empty container");
            }
        };

        // Reset focus
        let workspace = self.get_current_workspace();
        let workspace = workspace.get();

        {
            let container = workspace.get_container_focused();

            let container = container.get_mut();

            if let Some((_, window)) = container.get_focused_window() {
                let handle = self
                    .seat
                    .get_keyboard()
                    .expect("Should have a keyboard seat");

                let serial = SERIAL_COUNTER.next_serial();
                handle.set_focus(
                    &display.handle(),
                    Some(window.toplevel().wl_surface()),
                    serial,
                );
            }
        }

        let root = workspace.root();
        let mut root = root.get_mut();
        if !root.has_container() {
            let output = self.space.outputs().next().unwrap();
            let geo = self.space.output_geometry(output).unwrap();
            root.height = geo.size.h;
            root.width = geo.size.w;
        }
        root.redraw(&mut self.space);
    }

    pub fn handle_pointer_axis<I: InputBackend>(
        &mut self,
        display: &&mut Display<Wazemmes<B>>,
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

        let dh = &mut display.handle();
        self.seat.get_pointer().unwrap().axis(self, dh, frame);
    }

    pub fn handle_pointer_button<I: InputBackend>(
        &mut self,
        display: &&mut Display<Wazemmes<B>>,
        event: &<I as InputBackend>::PointerButtonEvent,
    ) {
        let dh = &mut display.handle();
        let pointer = self.seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let button_state = wl_pointer::ButtonState::from(event.state());

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

        if wl_pointer::ButtonState::Pressed == button_state {
            if let Some(window) = self.space.window_under(pointer.current_location()).cloned() {
                let window = WindowWarp::from(window);

                if self.mod_pressed && window.is_floating() {
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
                    let ws = self.get_current_workspace();
                    let mut ws = ws.get_mut();
                    let container = ws.root().container_having_window(id).unwrap();
                    self.set_window_focus(dh, serial, window.get());
                    let focused_id = container.get().id;
                    ws.set_container_focused(focused_id);
                }
            } else {
                self.space.windows().for_each(|window| {
                    window.set_activated(false);
                    window.configure();
                });

                let keyboard = self.seat.get_keyboard().unwrap();
                keyboard.set_focus(dh, None, serial);
            }
        };
    }

    fn set_window_focus(&mut self, dh: &mut DisplayHandle, serial: Serial, window: &Window) {
        let keyboard = self.seat.get_keyboard().unwrap();

        self.space.windows().for_each(|window| {
            window.set_activated(false);
            window.configure();
        });

        let window = WindowWarp::from(window.clone());
        self.space
            .map_window(window.get(), window.location(), window.z_index(), true);
        keyboard.set_focus(dh, Some(window.toplevel().wl_surface()), serial);
        window.get().set_activated(true);
        window.get().configure();
    }

    pub fn handle_pointer_motion<I: InputBackend>(
        &mut self,
        display: &&mut Display<Wazemmes<B>>,
        event: &<I as InputBackend>::PointerMotionAbsoluteEvent,
    ) {
        let output = self.space.outputs().next().unwrap();
        let geometry = self.space.output_geometry(output).unwrap();
        let pos = event.position_transformed(geometry.size) + geometry.loc.to_f64();
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

    pub fn set_layout_h(&mut self) {
        self.next_layout = Some(ContainerLayout::Horizontal)
    }

    pub fn set_layout_v(&mut self) {
        self.next_layout = Some(ContainerLayout::Vertical)
    }

    pub fn move_focus(&mut self, direction: Direction, display: &mut Display<Wazemmes<B>>) {
        let window = self.scan_window(direction);

        if let Some(window) = window {
            let serial = SERIAL_COUNTER.next_serial();
            let id = window.user_data().get::<WindowState>().unwrap().id();
            let ws = self.get_current_workspace();
            let mut ws = ws.get_mut();
            let container = ws.root().container_having_window(id).unwrap();
            let focused_id = container.get().id;
            ws.set_container_focused(focused_id);
            self.set_window_focus(&mut display.handle(), serial, &window);
        }
    }

    pub fn toggle_floating(&mut self) {
        let ws = self.get_current_workspace();
        let ws = ws.get();
        let container = ws.get_container_focused();
        let mut container = container.get_mut();

        if let Some((_id, window)) = container.get_focused_window_mut() {
            let output = self.space.outputs().next().unwrap();
            let geometry = self.space.output_geometry(output).unwrap();
            let y = geometry.size.h / 2 + geometry.loc.y;
            let x = geometry.size.w / 2 + geometry.loc.x;

            window.toggle_floating();
            self.space
                .map_window(window.get(), (x, y), FLOATING_Z_INDEX, true);
            window.get().configure();
            container.redraw(&mut self.space);
        }
    }

    fn scan_window(&mut self, direction: Direction) -> Option<Window> {
        let ws = self.get_current_workspace();
        let ws = ws.get();
        let container = ws.get_container_focused();
        let container = container.get();
        let mut window = None;

        if let Some((_idx, window_ref)) = container.get_focused_window() {
            let loc = self
                .space
                .window_location(window_ref.get())
                .expect("window should have a location");
            let (mut x, mut y) = (loc.x, loc.y);
            let width = window_ref.get().geometry().size.w;
            let height = window_ref.get().geometry().size.h;

            match direction {
                Direction::Left => y += height / 2,
                Direction::Right => {
                    x += width;
                    y += height / 2
                }
                Direction::Up => {
                    x += width / 2;
                }
                Direction::Down => {
                    x += width / 2;
                    y += height;
                }
            }

            let mut point = Point::from((x as f64, y as f64));

            while window.is_none() {
                if self.space.output_under(point).next().is_none() {
                    break;
                }

                direction.update_point(&mut point);

                window = {
                    let window = self.space.window_under(point);
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
