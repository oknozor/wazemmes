use std::sync::atomic::Ordering;
use std::time::Duration;

use slog::Logger;
use smithay::backend::renderer::gles2::Gles2Renderer;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::backend::SwapBuffersError;
use smithay::desktop::space::RenderError;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::protocol::{wl_output, wl_surface};
use smithay::reexports::wayland_server::Display;
use smithay::utils::IsAlive;
use smithay::wayland::output::{Mode, Output, PhysicalProperties};
use smithay::wayland::seat::CursorImageStatus;

use crate::drawing::{draw_cursor, draw_dnd_icon, CustomElem};

use crate::shell::workspace::WorkspaceRef;
use crate::{Backend, CallLoopData, Wazemmes};

pub struct WinitData {
    backend: WinitGraphicsBackend,
    full_redraw: u8,
}

impl Backend for WinitData {
    fn seat_name(&self) -> String {
        String::from("winit")
    }
    fn reset_buffers(&mut self, _output: &Output) {
        self.full_redraw = 4;
    }
    fn early_import(&mut self, _surface: &wl_surface::WlSurface) {}
}

pub fn init_winit(log: Logger) {
    let mut event_loop = EventLoop::try_new().unwrap();
    let mut display = Display::new().unwrap();

    let (backend, mut winit) = match winit::init(log.clone()) {
        Ok(ret) => ret,
        Err(err) => {
            panic!("Failed to initialize Winit backend: {}", err);
        }
    };

    let size = backend.window_size().physical_size;

    let data = {
        WinitData {
            backend,
            full_redraw: 0,
        }
    };

    let mut state = Wazemmes::new(event_loop.handle(), &mut display, data, log.clone());

    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: wl_output::Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
        log.clone(),
    );

    let _global = output.create_global::<Wazemmes<WinitData>>(&display.handle());
    output.change_current_state(
        Some(mode),
        Some(wl_output::Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));

    let start_time = std::time::Instant::now();

    slog::info!(log, "Initialization completed, starting the main loop.");

    state
        .workspaces
        .insert(0, WorkspaceRef::new(output.clone(), &mut state.space));

    while state.running.load(Ordering::SeqCst) {
        if winit
            .dispatch_new_events(|event| match event {
                WinitEvent::Resized { size, .. } => {
                    let _dh = display.handle();
                    // We only have one output
                    let output = state.space.outputs().next().unwrap().clone();
                    state.space.map_output(&output, (0, 0));
                    let mode = Mode {
                        size,
                        refresh: 60_000,
                    };
                    output.change_current_state(Some(mode), None, None, None);
                    output.set_preferred(mode);
                }
                WinitEvent::Input(event) => state.process_input_event(&mut display, event),
                _ => (),
            })
            .is_err()
        {
            state.running.store(false, Ordering::SeqCst);
            break;
        }

        // drawing logic
        {
            let backend = &mut state.backend_data.backend;
            let cursor_visible: bool;

            let mut elements = Vec::<CustomElem<Gles2Renderer>>::new();
            let mut cursor_guard = state.cursor_status.lock().unwrap();

            // draw the dnd icon if any
            if let Some(surface) = state.dnd_icon.as_ref() {
                if surface.alive() {
                    elements.push(
                        draw_dnd_icon(surface.clone(), state.pointer_location.to_i32_round(), &log)
                            .into(),
                    );
                }
            }

            // draw the cursor as relevant
            // reset the cursor if the surface is no longer alive
            let mut reset = false;
            if let CursorImageStatus::Image(ref surface) = *cursor_guard {
                reset = !surface.alive();
            }
            if reset {
                *cursor_guard = CursorImageStatus::Default;
            }
            if let CursorImageStatus::Image(ref surface) = *cursor_guard {
                cursor_visible = false;
                elements.push(
                    draw_cursor(surface.clone(), state.pointer_location.to_i32_round(), &log)
                        .into(),
                );
            } else {
                cursor_visible = true;
            }

            // draw FPS
            let full_redraw = &mut state.backend_data.full_redraw;
            *full_redraw = full_redraw.saturating_sub(1);
            let space = &mut state.space;
            let render_res = backend.bind().and_then(|_| {
                let renderer = backend.renderer();
                crate::render::render_output(&output, space, renderer, 0, &elements, &log).map_err(
                    |err| match err {
                        RenderError::Rendering(err) => err.into(),
                        _ => unreachable!(),
                    },
                )
            });

            let age = if *full_redraw > 0 {
                0
            } else {
                backend.buffer_age().unwrap_or(0)
            };

            match render_res {
                Ok(Some(damage)) => {
                    if let Err(err) = backend.submit(if age == 0 { None } else { Some(&*damage) }) {
                        slog::warn!(log, "Failed to submit buffer: {}", err);
                    }
                    backend.window().set_cursor_visible(cursor_visible);
                }
                Ok(None) => backend.window().set_cursor_visible(cursor_visible),
                Err(SwapBuffersError::ContextLost(err)) => {
                    slog::error!(log, "Critical Rendering Error: {}", err);
                    state.running.store(false, Ordering::SeqCst);
                }
                Err(err) => slog::warn!(log, "Rendering error: {}", err),
            }
        }

        // Send frame events so that client start drawing their next frame
        state
            .space
            .send_frames(start_time.elapsed().as_millis() as u32);

        let mut calloop_data = CallLoopData { state, display };
        let result = event_loop.dispatch(Some(Duration::from_millis(16)), &mut calloop_data);
        CallLoopData { state, display } = calloop_data;

        if result.is_err() {
            state.running.store(false, Ordering::SeqCst);
        } else {
            state.space.refresh(&display.handle());
            state.popups.cleanup();
            display.flush_clients().unwrap();
        }
    }
}
