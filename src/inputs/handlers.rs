use crate::shell::container::{ContainerLayout, ContainerState};
use slog::debug;
use smithay::backend::input::{
    Axis, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent,
    PointerButtonEvent, PointerMotionAbsoluteEvent,
};
use smithay::reexports::wayland_server::protocol::wl_pointer;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::wayland::seat::{AxisFrame, ButtonEvent, FilterResult, MotionEvent};
use smithay::wayland::SERIAL_COUNTER;

use crate::inputs::KeyAction;
use crate::shell::tree::ContainerRef;
use crate::state::Wazemmes;
use crate::Backend;
use smithay::wayland::seat::keysyms as xkb;

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
            let mut container = container.borrow_mut();
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
        let container = self
            .get_current_workspace()
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
        handle.set_focus(
            &display.handle(),
            Some(window.get_toplevel().wl_surface()),
            serial,
        );
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
        let keyboard = self.seat.get_keyboard().unwrap();
        let serial = SERIAL_COUNTER.next_serial();

        let button = event.button_code();

        let button_state = wl_pointer::ButtonState::from(event.state());

        if wl_pointer::ButtonState::Pressed == button_state && !pointer.is_grabbed() {
            if let Some(window) = self.space.window_under(pointer.current_location()).cloned() {
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

    pub fn handle_pointer_motion<I: InputBackend>(
        &mut self,
        display: &&mut Display<Wazemmes<B>>,
        event: &<I as InputBackend>::PointerMotionAbsoluteEvent,
    ) {
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

    pub fn set_layout_h(&mut self) {
        self.next_layout = Some(ContainerLayout::Horizontal)
    }

    pub fn set_layout_v(&mut self) {
        self.next_layout = Some(ContainerLayout::Vertical)
    }
}
