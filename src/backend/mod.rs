use slog_scope::info;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::input::{InputBackend, InputEvent};
use smithay::backend::renderer::gles2::{Gles2Renderer, Gles2Texture};
use smithay::backend::session::auto::AutoSession;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_protocols::wp::linux_dmabuf::zv1::server::zwp_linux_dmabuf_v1;
use smithay::reexports::wayland_server::protocol::wl_output;
use smithay::reexports::wayland_server::{DisplayHandle, GlobalDispatch};
use smithay::utils::{Physical, Rectangle};
use smithay::wayland;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::dmabuf::{
    DmabufGlobal, DmabufGlobalData, DmabufHandler, DmabufState, ImportError,
};
use smithay::wayland::output;
use smithay::wayland::output::PhysicalProperties;
use std::str::FromStr;

pub mod drawing;
pub mod drm;
pub mod libinput;
pub mod winit;
#[cfg(feature = "xwayland")]
pub mod xwayland;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct OutputId {
    id: u64,
}

#[derive(Debug)]
pub struct NewOutputDescriptor {
    pub id: OutputId,
    pub name: String,
    pub physical_properties: PhysicalProperties,

    pub prefered_mode: output::Mode,
    pub possible_modes: Vec<output::Mode>,

    pub transform: wl_output::Transform,
}

pub enum BackendState {
    Drm(drm::DrmBackendState),
    None,
}

impl Default for BackendState {
    fn default() -> Self {
        Self::None
    }
}

impl BackendState {
    fn init_drm(&mut self, inner: drm::DrmBackendState) {
        *self = Self::Drm(inner);
    }

    fn drm(&mut self) -> &mut drm::DrmBackendState {
        if let Self::Drm(i) = self {
            i
        } else {
            unreachable!("Only one backend at the time");
        }
    }
}

impl BackendState {
    pub fn update_mode(&mut self, output_id: &OutputId, mode: &wayland::output::Mode) {
        match self {
            BackendState::Drm(state) => state.update_mode(output_id, mode),
            BackendState::None => {}
        }
    }

    pub fn dmabuf_imported(
        &mut self,
        dh: &DisplayHandle,
        global: &DmabufGlobal,
        dmabuf: Dmabuf,
    ) -> Result<(), ImportError> {
        match self {
            BackendState::Drm(state) => state.dmabuf_imported(dh, global, dmabuf),
            BackendState::None => Ok(()),
        }
    }
}

pub trait BackendHandler: OutputHandler + InputHandler {
    type WaylandState: GlobalDispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, DmabufGlobalData>
        + BufferHandler
        + DmabufHandler
        + 'static;

    fn dmabuf_state(&mut self) -> &mut DmabufState;
    fn backend_state(&mut self) -> &mut BackendState;
    #[cfg(feature = "xwayland")]
    fn start_xwayland(&mut self);
    fn start_compositor(&mut self);
    fn close_compositor(&mut self);
}

pub trait InputHandler {
    fn process_input_event<I: InputBackend>(
        &mut self,
        event: InputEvent<I>,
        absolute_output: Option<&OutputId>,
        session: Option<&mut AutoSession>,
    );
}

pub trait OutputHandler {
    fn output_created(&mut self, output: NewOutputDescriptor);

    fn output_mode_updated(&mut self, output_id: &OutputId, mode: wayland::output::Mode);

    fn output_render(
        &mut self,
        renderer: &mut Gles2Renderer,
        output: &OutputId,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Physical>>>, smithay::backend::SwapBuffersError>;

    fn send_frames(&mut self, output_id: &OutputId);
}

#[derive(Debug)]
pub enum PreferedBackend {
    X11,
    Winit,
    Udev,
    Auto,
}

#[derive(Debug)]
pub struct PreferedBackendParseError(String);

impl std::fmt::Display for PreferedBackendParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unknown backend: {}", self.0)
    }
}

impl std::error::Error for PreferedBackendParseError {}

impl FromStr for PreferedBackend {
    type Err = PreferedBackendParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "x11" => Self::X11,
            "winit" => Self::Winit,
            "udev" => Self::Udev,
            "auto" => Self::Auto,
            other => return Err(PreferedBackendParseError(other.to_string())),
        })
    }
}

pub fn init<D>(
    event_loop: &mut EventLoop<'static, D>,
    display: &DisplayHandle,
    handler: &mut D,
    backend: PreferedBackend,
) where
    D: BackendHandler + AsMut<DmabufState> + 'static,
{
    match backend {
        PreferedBackend::Auto => {
            if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
                info!("Starting with winit backend");
                #[cfg(feature = "winit")]
                winit::run_winit(event_loop, display, handler)
                    .expect("Failed to initialize winit backend.");
            } else {
                info!("Starting with udev backend");
                #[cfg(feature = "udev")]
                drm::run_udev(event_loop, display, handler)
                    .expect("Failed to initialize tty backend.");
            }
        }
        PreferedBackend::X11 => {
            todo!()
        }
        PreferedBackend::Winit =>
        {
            #[cfg(feature = "winit")]
            winit::run_winit(event_loop, display, handler)
                .expect("Failed to initialize winit backend.")
        }
        PreferedBackend::Udev => {
            #[cfg(feature = "udev")]
            drm::run_udev(event_loop, display, handler).expect("Failed to initialize tty backend.");
        }
    }
}
