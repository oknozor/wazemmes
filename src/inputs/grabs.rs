use crate::backend::drawing::FLOATING_Z_INDEX;
use crate::shell::windows::toplevel::WindowWrap;
use crate::shell::workspace;
use crate::Wazemmes;
use smithay::desktop::Window;
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, GrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
};
use smithay::input::SeatHandler;
use smithay::utils::{Logical, Point};

pub struct MoveSurfaceGrab {
    pub start_data: GrabStartData<Wazemmes>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<Wazemmes> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut Wazemmes,
        handle: &mut PointerInnerHandle<'_, Wazemmes>,
        focus: Option<(<Wazemmes as SeatHandler>::PointerFocus, Point<i32, Logical>)>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(data, focus, event);

        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;
        let location = new_location.to_i32_round();
        let window = WindowWrap::from(self.window.clone());
        window.update_loc(location);
        window.map(&mut data.space, data.x11_state.as_mut(), true);
        let ws = data.get_current_workspace();
        let mut ws = ws.get_mut();
        ws.needs_redraw = true;
    }

    fn button(
        &mut self,
        data: &mut Wazemmes,
        handle: &mut PointerInnerHandle<'_, Wazemmes>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        if handle.current_pressed().is_empty() {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(data, event.serial, event.time);
        }
    }

    fn axis(
        &mut self,
        data: &mut Wazemmes,
        handle: &mut PointerInnerHandle<'_, Wazemmes>,
        details: AxisFrame,
    ) {
        handle.axis(data, details)
    }

    fn start_data(&self) -> &GrabStartData<Wazemmes> {
        &self.start_data
    }
}
