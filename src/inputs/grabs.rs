use crate::shell::window::{WindowWrap, FLOATING_Z_INDEX};
use crate::{Backend, Wazemmes};
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

impl<BackendData: Backend> PointerGrab<Wazemmes<BackendData>> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut Wazemmes<BackendData>,
        _dh: &DisplayHandle,
        handle: &mut PointerInnerHandle<'_, Wazemmes<BackendData>>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(event.location, None, event.serial, event.time);

        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;
        let location = new_location.to_i32_round();

        // Save the current window location
        WindowWrap::from(self.window.clone())
            .get_state()
            .set_location(location);

        data.space
            .map_window(&self.window, location, FLOATING_Z_INDEX, true);
    }

    fn button(
        &mut self,
        _data: &mut Wazemmes<BackendData>,
        _dh: &DisplayHandle,
        handle: &mut PointerInnerHandle<'_, Wazemmes<BackendData>>,
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
        _data: &mut Wazemmes<BackendData>,
        _dh: &DisplayHandle,
        handle: &mut PointerInnerHandle<'_, Wazemmes<BackendData>>,
        details: AxisFrame,
    ) {
        handle.axis(details)
    }

    fn start_data(&self) -> &PointerGrabStartData {
        &self.start_data
    }
}
