mod compositor;
mod xdg_decoration;
mod xdg_shell;

use crate::{Backend, Wazemmes};

//
// Wl Seat
//

use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::wayland::primary_selection::{PrimarySelectionHandler, PrimarySelectionState};
use smithay::wayland::seat::{SeatHandler, SeatState};
use smithay::{delegate_data_device, delegate_output, delegate_primary_selection, delegate_seat};

impl<BackendData: Backend> SeatHandler for Wazemmes<BackendData> {
    fn seat_state(&mut self) -> &mut SeatState<Wazemmes<BackendData>> {
        &mut self.seat_state
    }
}

delegate_seat!(@<BackendData: 'static + Backend> Wazemmes<BackendData>);

//
// Wl Data Device
//

impl<BackendData: Backend> DataDeviceHandler for Wazemmes<BackendData> {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.data_device_state
    }
}

impl<BackendData: Backend> ClientDndGrabHandler for Wazemmes<BackendData> {}
impl<BackendData: Backend> ServerDndGrabHandler for Wazemmes<BackendData> {}

delegate_data_device!(@<BackendData: 'static + Backend> Wazemmes<BackendData>);

//
// Wl Output & Xdg Output
//

delegate_output!(@<BackendData: 'static + Backend> Wazemmes<BackendData>);

impl<BackendData: Backend> PrimarySelectionHandler for Wazemmes<BackendData> {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}
delegate_primary_selection!(@<BackendData: 'static + Backend> Wazemmes<BackendData>);
