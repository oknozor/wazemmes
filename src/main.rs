#![feature(drain_filter)]
#![feature(hash_drain_filter)]

use crate::backend::BackendState;
use crate::config::WazemmesConfig;
use crate::resources::pointer::PointerIcon;
use crate::shell::workspace::WorkspaceRef;
use crate::state::{CallLoopData, Wazemmes};
use clap::Parser;
use slog::Drain;
use slog_scope::error;
use smithay::desktop;
use smithay::desktop::PopupManager;
use smithay::input::SeatState;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::wayland::compositor::CompositorState;
use smithay::wayland::data_device::DataDeviceState;
use smithay::wayland::dmabuf::DmabufState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::primary_selection::PrimarySelectionState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "xwayland")]
use smithay::xwayland::{XWayland, XWaylandEvent};
use crate::handlers::screencopy::ScreenCopyManagerState;

mod backend;
pub mod border;
mod cli;
mod config;
pub mod draw;
mod handlers;
mod inputs;
mod resources;
mod shell;
pub mod state;

fn init_log() -> slog::Logger {
    let terminal_drain = slog_envlogger::LogBuilder::new(
        slog_term::CompactFormat::new(slog_term::TermDecorator::new().stderr().build())
            .build()
            .fuse(),
    )
    .filter(Some("wazemmes"), slog::FilterLevel::Trace)
    .filter(Some("smithay"), slog::FilterLevel::Warning)
    .build()
    .fuse();

    let terminal_drain = slog_async::Async::default(terminal_drain).fuse();

    let log = slog::Logger::root(terminal_drain.fuse(), slog::o!());

    slog_stdlog::init().expect("Could not setup log backend");

    log
}

struct ClientState;
impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

#[cfg(feature = "xwayland")]
fn init_xwayland_connection(
    handle: &LoopHandle<'static, CallLoopData>,
    display: &DisplayHandle,
) -> XWayland {
    let xwayland = {
        let (xwayland, channel) = XWayland::new(slog_scope::logger(), display);
        let ret = handle.insert_source(channel, |event, _, data| match event {
            XWaylandEvent::Ready {
                connection, client, ..
            } => data.state.xwayland_ready(connection, client),
            XWaylandEvent::Exited => data.state.xwayland_exited(),
        });
        if let Err(e) = ret {
            error!(
                "Failed to insert the XWaylandSource into the event loop: {}",
                e
            );
        }
        xwayland
    };

    xwayland
}

fn init_wayland_listener<D>(
    display: &mut Display<D>,
    event_loop: &mut EventLoop<CallLoopData>,
    log: slog::Logger,
) -> OsString {
    // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
    let listening_socket = ListeningSocketSource::new_auto(log).unwrap();

    // Get the name of the listening socket.
    // Clients will connect to this socket.
    let socket_name = listening_socket.socket_name().to_os_string();

    let handle = event_loop.handle();

    event_loop
        .handle()
        .insert_source(listening_socket, move |client_stream, _, state| {
            // Inside the callback, you should insert the client into the display.
            //
            // You may also associate some data with the client when inserting the client.
             state
                .display
                .handle()
                .insert_client(client_stream, Arc::new(ClientState))
                .unwrap();
       })
        .expect("Failed to init the wayland event source.");
    

    // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
    handle
        .insert_source(
            Generic::new(display.backend().poll_fd(), Interest::READ, Mode::Level),
            |_, _, state| {
                state.display.dispatch_clients(&mut state.state).unwrap();
                Ok(PostAction::Continue)
            },
        )
        .unwrap();

    socket_name
}

fn main() -> eyre::Result<()> {
    let log = init_log();
    let _guard = slog_scope::set_global_logger(log);

    let opt = cli::WazemmesCli::parse();

    let mut event_loop = EventLoop::<CallLoopData>::try_new()?;
    let mut display = Display::new()?;
    let socket_name = init_wayland_listener(&mut display, &mut event_loop, slog_scope::logger());
    let pointer_icon = PointerIcon::new();
    let dh = display.handle();
    let compositor_state = CompositorState::new::<Wazemmes, _>(&dh, slog_scope::logger());
    let xdg_shell_state = XdgShellState::new::<Wazemmes, _>(&dh, slog_scope::logger());
    let shm_state = ShmState::new::<Wazemmes, _>(&dh, vec![], slog_scope::logger());
    let output_manager_state = OutputManagerState::new_with_xdg_output::<Wazemmes>(&dh);
    let primary_selection_state =
        PrimarySelectionState::new::<Wazemmes, _>(&dh, slog_scope::logger());
    let mut seat_state = SeatState::<Wazemmes>::new();
    let data_device_state = DataDeviceState::new::<Wazemmes, _>(&dh, slog_scope::logger());
    let xdg_decoration_state = XdgDecorationState::new::<Wazemmes, _>(&dh, slog_scope::logger());
    let screen_copy_manager_state = ScreenCopyManagerState::new(&dh);

    let dmabuf_state = DmabufState::new();

    let mut seat = seat_state.new_wl_seat(&display.handle(), "seat0", slog_scope::logger());

    seat.add_pointer();

    seat.add_keyboard(Default::default(), 200, 200)?;

    #[cfg(feature = "xwayland")]
    let xwayland = init_xwayland_connection(&event_loop.handle(), &display.handle());

    let state = Wazemmes {
        space: desktop::Space::new(slog_scope::logger()),
        popups: PopupManager::new(slog_scope::logger()),
        display: display.handle(),
        start_time: Instant::now(),
        loop_signal: event_loop.get_signal(),
        _loop_handle: event_loop.handle(),
        seat,
        compositor_state,
        xdg_shell_state,
        xdg_decoration_state,
        primary_selection_state,
        shm_state,
        _output_manager_state: output_manager_state,
        screen_copy_manager_state: screen_copy_manager_state,
        seat_state,
        data_device_state,
        dmabuf_state,
        pointer_icon,
        backend: BackendState::default(),
        socket_name,

        #[cfg(feature = "xwayland")]
        x11_state: None,
        #[cfg(feature = "xwayland")]
        xwayland,

        // Shell
        workspaces: Default::default(),
        current_workspace: 0,
        next_layout: None,
        mod_pressed: false,
    };

    let config = WazemmesConfig::get()?;
    let mut data = CallLoopData {
        state,
        config,
        display,
    };

    backend::init(
        &mut event_loop,
        &data.display.handle(),
        &mut data,
        opt.backend,
    );

    #[cfg(feature = "xwayland")]
    data.state.xwayland.start(event_loop.handle())?;

    event_loop.run(None, &mut data, |data| {
        data.state.space.refresh(&data.display.handle());
        data.state.popups.cleanup();
        data.display.flush_clients().unwrap();
    })?;

    Ok(())
}
