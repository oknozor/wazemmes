mod compositor;
mod xdg_shell;

use crate::Wazemmes;

//
// Wl Seat
//

use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::wayland::seat::{SeatHandler, SeatState};
use smithay::{delegate_data_device, delegate_output, delegate_seat};

impl SeatHandler for Wazemmes {
    fn seat_state(&mut self) -> &mut SeatState<Wazemmes> {
        &mut self.seat_state
    }
}

delegate_seat!(Wazemmes);

//
// Wl Data Device
//

impl DataDeviceHandler for Wazemmes {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for Wazemmes {}
impl ServerDndGrabHandler for Wazemmes {}

delegate_data_device!(Wazemmes);

//
// Wl Output & Xdg Output
//

delegate_output!(Wazemmes);
