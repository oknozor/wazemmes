mod compositor;
mod xdg_shell;

use crate::Wazemmes;

//
// Wl Seat
//

use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::wayland::primary_selection::{PrimarySelectionHandler, PrimarySelectionState};
use smithay::wayland::seat::{SeatHandler, SeatState};
use smithay::{delegate_data_device, delegate_output, delegate_primary_selection, delegate_seat};

impl<Backend> SeatHandler for Wazemmes<Backend> {
    fn seat_state(&mut self) -> &mut SeatState<Wazemmes<Backend>> {
        &mut self.seat_state
    }
}

delegate_seat!(@<BackendData: 'static> Wazemmes<BackendData>);

//
// Wl Data Device
//

impl<Backend> DataDeviceHandler for Wazemmes<Backend> {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.data_device_state
    }
}

impl<Backend> ClientDndGrabHandler for Wazemmes<Backend> {}
impl<Backend> ServerDndGrabHandler for Wazemmes<Backend> {}

delegate_data_device!(@<BackendData: 'static> Wazemmes<BackendData>);

//
// Wl Output & Xdg Output
//

delegate_output!(@<BackendData: 'static> Wazemmes<BackendData>);

impl<BackendData> PrimarySelectionHandler for Wazemmes<BackendData> {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}
delegate_primary_selection!(@<BackendData: 'static> Wazemmes<BackendData>);
