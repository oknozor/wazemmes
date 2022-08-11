use crate::shell::container::{ContainerLayout, ContainerState};
use slog::debug;
use smithay::backend::input::{
    Axis, Event, InputBackend, PointerAxisEvent, PointerButtonEvent, PointerMotionAbsoluteEvent,
};
use smithay::desktop::{Window};
use smithay::reexports::wayland_server::protocol::wl_pointer;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Logical, Point};
use smithay::wayland::seat::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::wayland::{Serial, SERIAL_COUNTER};



use crate::shell::container::ContainerRef;
use crate::shell::window::WindowId;
use crate::state::Wazemmes;
use crate::{Backend};

impl<B: Backend> Wazemmes<B> {
    pub fn run(cmd: String) {
        std::process::Command::new(cmd).spawn().ok();
    }

    pub fn close(&mut self, display: &&mut Display<Wazemmes<B>>) {
        let state = {
            let container = self
                .get_current_workspace()
                .get_mut()
                .tree
                .get_container_focused();
            let mut container = container.get_mut();
            debug!(&self.log, "Closing window in container: {}", container.id);
            container.close_window();
            container.state()
        };

        match state {
            ContainerState::Empty => {
                println!("empty container removed");
                self.get_current_workspace().get_mut().tree.pop();
            }
            ContainerState::HasChildrenOnly => {
                let container = self
                    .get_current_workspace()
                    .get_mut()
                    .tree
                    .get_container_focused();
                let copy = container.clone();
                let mut container = container.get_mut();
                let id = container.id;
                if let Some(parent) = &mut container.parent {
                    let childs: Vec<ContainerRef> = copy.get_mut().childs.drain(..).collect();
                    let mut parent = parent.get_mut();
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
        let workspace = self.get_current_workspace();
        let workspace = workspace.get();

        {
            let container = workspace.tree.get_container_focused();

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

        let root = workspace.tree.root();
        let root = root.get();
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

        if wl_pointer::ButtonState::Pressed == button_state && !pointer.is_grabbed() {
            if let Some(window) = self.space.window_under(pointer.current_location()).cloned() {
                let id = window.user_data().get::<WindowId>().unwrap();

                self.set_window_focus(dh, serial, &window);
                self.set_container_focus(id.get());
            } else {
                self.space.windows().for_each(|window| {
                    window.set_activated(false);
                    window.configure();
                });

                let keyboard = self.seat.get_keyboard().unwrap();
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

    fn set_container_focus(&self, window_id: u32) {
        let ws = self.get_current_workspace();
        let tree = &mut ws.get_mut().tree;
        let container = tree
            .root()
            .container_having_window(window_id)
            .expect("Window should have a container");

        container.get_mut().set_focus(window_id);

        tree.set_container_focused(container);
    }

    fn set_window_focus(&mut self, dh: &mut DisplayHandle, serial: Serial, window: &Window) {
        let keyboard = self.seat.get_keyboard().unwrap();

        self.space.windows().for_each(|window| {
            window.set_activated(false);
            window.configure();
        });

        self.space.raise_window(window, true);

        keyboard.set_focus(dh, Some(window.toplevel().wl_surface()), serial);
        window.set_activated(true);
        window.configure();
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
            let id = window.user_data().get::<WindowId>().unwrap().get();

            self.set_container_focus(id);
            self.set_window_focus(&mut display.handle(), serial, &window);
        }
    }

    fn scan_window(&mut self, direction: Direction) -> Option<Window> {
        let ws = self.get_current_workspace();
        let ws = ws.get();
        let container = ws.tree.get_container_focused();
        let container = container.get();
        let mut window = None;
        if let Some((_idx, window_ref)) = container.get_focused_window() {
            let loc = self
                .space
                .window_location(window_ref.get())
                .expect("window should have a location");
            let mut x = loc.x;
            let mut y = loc.y;
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

            loop {
                direction.update_point(&mut point);

                if self.space.output_under(point).next().is_none() {
                    break;
                }

                window = {
                    let window = self.space.window_under(point);
                    window.cloned()
                };

                if window.is_some() {
                    break;
                }
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
