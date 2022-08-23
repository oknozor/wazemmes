use crate::shell::window::WindowWrap;
use crate::Wazemmes;
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{layer_map_for_output, Kind as SurfaceKind, Space, WindowSurfaceType};

use smithay::reexports::wayland_server::protocol::wl_buffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    with_states, with_surface_tree_upward, CompositorHandler, CompositorState, TraversalAction,
};
use smithay::wayland::shell::wlr_layer::LayerSurfaceAttributes;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceRoleAttributes;
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::wayland::Serial;
use smithay::{delegate_compositor, delegate_shm};
use std::cell::RefCell;
use std::sync::Mutex;

/// State of the resize operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ResizeState {
    /// The surface is not being resized.
    NotResizing,
    /// The surface is currently being resized.
    Resizing(ResizeData),
    /// The resize has finished, and the surface needs to ack the final configure.
    WaitingForFinalAck(ResizeData, Serial),
    /// The resize has finished, and the surface needs to commit its final state.
    WaitingForCommit(ResizeData),
}

/// Information about the resize operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ResizeData {
    /// The edges the surface is being resized with.
    edges: ResizeEdge,
    /// The initial window location.
    initial_window_location: Point<i32, Logical>,
    /// The initial window size (geometry width and height).
    initial_window_size: Size<i32, Logical>,
}

bitflags::bitflags! {
    struct ResizeEdge: u32 {
        const NONE = 0;
        const TOP = 1;
        const BOTTOM = 2;
        const LEFT = 4;
        const TOP_LEFT = 5;
        const BOTTOM_LEFT = 6;
        const RIGHT = 8;
        const TOP_RIGHT = 9;
        const BOTTOM_RIGHT = 10;
    }
}

impl Default for ResizeState {
    fn default() -> Self {
        ResizeState::NotResizing
    }
}

#[derive(Default)]
pub struct SurfaceData {
    pub geometry: Option<Rectangle<i32, Logical>>,
    pub resize_state: ResizeState,
}

impl CompositorHandler for Wazemmes {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler(surface);
        self.space.commit(surface);
        ensure_initial_configure(&self.display, surface, &mut self.space);
    }
}

fn ensure_initial_configure(dh: &DisplayHandle, surface: &WlSurface, space: &mut Space) {
    with_surface_tree_upward(
        surface,
        (),
        |_, _, _| TraversalAction::DoChildren(()),
        |_, states, _| {
            states
                .data_map
                .insert_if_missing(|| RefCell::new(SurfaceData::default()));
        },
        |_, _, _| true,
    );
    if let Some(window) = space
        .window_for_surface(surface, WindowSurfaceType::TOPLEVEL)
        .cloned()
    {
        let window = WindowWrap::from(window);

        // send the initial configure if relevant
        #[cfg_attr(not(feature = "xwayland"), allow(irrefutable_let_patterns))]
        if let SurfaceKind::Xdg(ref toplevel) = window.get().toplevel() {
            let (initial_configure_sent, configured) = with_states(surface, |states| {
                let attributes = states
                    .data_map
                    .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                    .unwrap()
                    .lock()
                    .unwrap();

                (attributes.initial_configure_sent, attributes.configured)
            });

            if initial_configure_sent && !configured {
                // We need to check the initial size before storing it
                // some client will send their initial size after configuration
                let geometry = window.get().geometry();
                if geometry.size.w != 0 && geometry.size.h != 0 {
                    window.get_state().set_initial_geometry(geometry.size);
                }
            } else if !initial_configure_sent {
                toplevel.send_configure();
            } else if configured && !window.get_state().configured() {
                let geometry = window.get().geometry();
                window.get_state().set_initial_geometry(geometry.size);
                with_states(surface, |states| {
                    let attributes = states
                        .data_map
                        .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                        .unwrap()
                        .lock()
                        .unwrap();

                    if let Some(app_id) = &attributes.app_id {
                        // TODO: configurable criteria
                        if app_id == "onagre" {
                            window.toggle_floating();
                        }
                    }

                    window.get_state().set_configured();
                });
            }
        }

        with_states(surface, |states| {
            let mut data = states
                .data_map
                .get::<RefCell<SurfaceData>>()
                .unwrap()
                .borrow_mut();

            // Finish resizing.
            if let ResizeState::WaitingForCommit(_) = data.resize_state {
                data.resize_state = ResizeState::NotResizing;
            }
        });

        return;
    }

    if let Some(output) = space.outputs().find(|o| {
        let map = layer_map_for_output(o);
        map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .is_some()
    }) {
        let mut map = layer_map_for_output(output);
        let layer = map
            .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .unwrap();

        // send the initial configure if relevant
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<Mutex<LayerSurfaceAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        if !initial_configure_sent {
            layer.layer_surface().send_configure();
        }

        map.arrange(dh);
    };
}

impl BufferHandler for Wazemmes {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for Wazemmes {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(Wazemmes);
delegate_shm!(Wazemmes);
