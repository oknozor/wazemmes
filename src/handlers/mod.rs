pub mod backend;
mod compositor;
pub mod dmabuf;
pub mod output;
mod xdg_decoration;
mod xdg_shell;

use crate::Wazemmes;

//
// Wl Seat
//

use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};

use smithay::{delegate_data_device, delegate_output};

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
