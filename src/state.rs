use crate::shell::container::ContainerLayout;
use crate::shell::workspace::WorkspaceRef;
use crate::{CallLoopData, WinitData};
use slog::Logger;
use smithay::desktop::{PopupManager, Space, WindowSurfaceType};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, Resource};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::CompositorState;
use smithay::wayland::data_device::{set_data_device_focus, DataDeviceState};
use smithay::wayland::output::{Output, OutputManagerState};
use smithay::wayland::primary_selection::{set_primary_focus, PrimarySelectionState};
use smithay::wayland::seat::{CursorImageStatus, PointerHandle, Seat, SeatState, XkbConfig};
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::tablet_manager::TabletSeatTrait;

use smithay::backend::renderer::gles2::Gles2Renderer;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub struct Wazemmes<BackendData: 'static + Backend> {
    pub backend_data: BackendData,
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub space: Space,
    pub log: Logger,

    // Desktop
    pub popups: PopupManager,

    // Smithay State
    pub running: Arc<AtomicBool>,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Wazemmes<BackendData>>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub xdg_decoration_state: XdgDecorationState,
    pub cursor_status: Arc<Mutex<CursorImageStatus>>,
    pub dnd_icon: Option<WlSurface>,
    pub pointer_location: Point<f64, Logical>,

    // Tree
    pub workspaces: HashMap<u8, WorkspaceRef>,
    pub current_workspace: u8,
    pub next_layout: Option<ContainerLayout>,
    pub mod_pressed: bool,

    // Seat
    pub seat: Seat<Self>,
}

impl<B: Backend> Wazemmes<B> {
    pub fn get_buffer_age(&mut self) -> usize {
        let backend = &mut self.backend_data;
        let full_redraw = backend.full_redraw();

        if full_redraw > 0 {
            0
        } else {
            backend.buffer_age().unwrap_or(0)
        }
    }

    pub fn new(
        handle: LoopHandle<CallLoopData<WinitData>>,
        display: &mut Display<Self>,
        backend_data: B,
        log: Logger,
    ) -> Self {
        // init wayland clients
        let socket_name = {
            let source = ListeningSocketSource::new_auto(log.clone()).unwrap();
            let socket_name = source.socket_name().to_string_lossy().into_owned();
            handle
                .insert_source(source, |client_stream, _, data| {
                    if let Err(err) = data
                        .display
                        .handle()
                        .insert_client(client_stream, Arc::new(ClientState))
                    {
                        slog::warn!(data.state.log, "Error adding wayland client: {}", err);
                    };
                })
                .expect("Failed to init wayland socket source");
            slog::info!(log, "Listening on wayland socket"; "name" => socket_name.clone());
            ::std::env::set_var("WAYLAND_DISPLAY", &socket_name);
            socket_name
        };
        handle
            .insert_source(
                Generic::new(display.backend().poll_fd(), Interest::READ, Mode::Level),
                |_, _, data| {
                    data.display.dispatch_clients(&mut data.state).unwrap();
                    Ok(PostAction::Continue)
                },
            )
            .expect("Failed to init wayland server source");

        // init globals
        let dh = display.handle();
        let compositor_state = CompositorState::new::<Self, _>(&dh, log.clone());
        let data_device_state = DataDeviceState::new::<Self, _>(&dh, log.clone());
        let output_manager_state = OutputManagerState::new();
        let seat_state = SeatState::new();
        let shm_state = ShmState::new::<Self, _>(&dh, vec![], log.clone());
        let xdg_shell_state = XdgShellState::new::<Self, _>(&dh, log.clone());
        let primary_selection_state = PrimarySelectionState::new::<Self, _>(&dh, log.clone());
        let xdg_decoration_state = XdgDecorationState::new::<Self, _>(&dh, log.clone());

        // init input
        let seat_name = backend_data.seat_name();
        let mut seat = Seat::new(&dh, seat_name, log.clone());

        let cursor_status = Arc::new(Mutex::new(CursorImageStatus::Default));
        let cursor_status2 = cursor_status.clone();
        seat.add_pointer(move |new_status| *cursor_status2.lock().unwrap() = new_status);

        seat.add_keyboard(XkbConfig::default(), 200, 25, move |seat, surface| {
            let focus = surface.and_then(|s| dh.get_client(s.id()).ok());
            let focus2 = surface.and_then(|s| dh.get_client(s.id()).ok());
            set_data_device_focus(&dh, seat, focus);
            set_primary_focus(&dh, seat, focus2);
        })
        .expect("Failed to initialize the keyboard");

        let cursor_status3 = cursor_status.clone();
        seat.tablet_seat()
            .on_cursor_surface(move |_tool, new_status| {
                // TODO: tablet tools should have their own cursors
                *cursor_status3.lock().unwrap() = new_status;
            });

        let popup_manager = PopupManager::new(log.clone());

        Wazemmes {
            backend_data,
            socket_name: socket_name.into(),
            space: Space::new(log.clone()),
            compositor_state,
            data_device_state,
            primary_selection_state,
            xdg_decoration_state,
            cursor_status,
            dnd_icon: None,
            pointer_location: (0.0, 0.0).into(),
            workspaces: HashMap::new(),
            current_workspace: 0,
            output_manager_state,
            seat_state,
            shm_state,
            xdg_shell_state,
            log,
            seat,
            start_time: std::time::Instant::now(),
            next_layout: None,
            running: Arc::new(AtomicBool::new(true)),
            popups: popup_manager,
            mod_pressed: false,
        }
    }

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
    fn full_redraw(&mut self) -> u8;
    fn buffer_age(&self) -> Option<usize>;
    fn renderer(&mut self) -> &mut Gles2Renderer;
    fn seat_name(&self) -> String;
    fn reset_buffers(&mut self, output: &Output);
    fn early_import(&mut self, surface: &WlSurface);
}
