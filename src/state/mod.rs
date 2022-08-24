use crate::shell::container::ContainerLayout;
use crate::shell::workspace::WorkspaceRef;

use smithay::desktop::WindowSurfaceType;

use smithay::reexports::calloop::{LoopHandle, LoopSignal};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::CompositorState;
use smithay::wayland::data_device::DataDeviceState;
use smithay::wayland::output::{Output, OutputManagerState};

use smithay::wayland::seat::{PointerHandle, Seat, SeatState};
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;

use crate::backend::BackendState;
use crate::resources::pointer::PointerIcon;
use smithay::desktop;
use smithay::wayland::dmabuf::DmabufState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;

use std::collections::HashMap;
use std::ffi::OsString;

use crate::backend::xwayland::X11State;
use crate::config::WazemmesConfig;
use smithay::xwayland::XWayland;
use std::time::Instant;

pub mod output;
pub mod seat;

pub struct Wazemmes {
    pub space: desktop::Space,
    pub display: DisplayHandle,
    pub start_time: Instant,
    pub loop_signal: LoopSignal,
    pub _loop_handle: LoopHandle<'static, CallLoopData>,
    pub seat: Seat<Self>,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub shm_state: ShmState,
    pub _output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: DataDeviceState,
    pub dmabuf_state: DmabufState,
    pub pointer_icon: PointerIcon,
    pub backend: BackendState,
    pub socket_name: OsString,

    #[cfg(feature = "xwayland")]
    pub xwayland: XWayland,
    #[cfg(feature = "xwayland")]
    pub x11_state: Option<X11State>,

    // Shell
    pub mod_pressed: bool,
    pub workspaces: HashMap<u8, WorkspaceRef>,
    pub current_workspace: u8,
    pub next_layout: Option<ContainerLayout>,
    pub config: WazemmesConfig,
}

impl Wazemmes {
    pub fn surface_under_pointer(
        &self,
        pointer: &PointerHandle<Self>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        let pos = pointer.current_location();
        self.space
            .surface_under(pos, WindowSurfaceType::all())
            .map(|(_, surface, location)| (surface, location))
    }
}

pub struct ClientState;

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

pub trait Backend {
    fn seat_name(&self) -> String;
    fn reset_buffers(&mut self, output: &Output);
    fn early_import(&mut self, surface: &WlSurface);
}

pub struct CallLoopData {
    pub state: Wazemmes,
    pub display: Display<Wazemmes>,
}
