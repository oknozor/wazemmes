use crate::Wazemmes;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point};
use smithay::wayland::seat::{
    AxisFrame, ButtonEvent, MotionEvent, PointerGrab, PointerGrabStartData, PointerInnerHandle,
};

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl<Backend> PointerGrab<Wazemmes<Backend>> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut Wazemmes<Backend>,
        _dh: &DisplayHandle,
        handle: &mut PointerInnerHandle<'_, Wazemmes<Backend>>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(event.location, None, event.serial, event.time);

        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;

        data.space
            .map_window(&self.window, new_location.to_i32_round(), None, true);
    }

    fn button(
        &mut self,
        _data: &mut Wazemmes<Backend>,
        _dh: &DisplayHandle,
        handle: &mut PointerInnerHandle<'_, Wazemmes<Backend>>,
        event: &ButtonEvent,
    ) {
        handle.button(event.button, event.state, event.serial, event.time);
        if handle.current_pressed().is_empty() {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(event.serial, event.time);
        }
    }

    fn axis(
        &mut self,
        _data: &mut Wazemmes<Backend>,
        _dh: &DisplayHandle,
        handle: &mut PointerInnerHandle<'_, Wazemmes<Backend>>,
        details: AxisFrame,
    ) {
        handle.axis(details)
    }

    fn start_data(&self) -> &PointerGrabStartData {
        &self.start_data
    }
}
