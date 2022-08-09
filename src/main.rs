#![feature(drain_filter)]

use crate::shell::tree::Tree;
use crate::state::Wazemmes;
use slog::Drain;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

mod handlers;
mod inputs;
mod shell;
mod state;
// mod udev;
mod winit;

pub struct CallLoopData {
    state: Wazemmes,
    display: Display<Wazemmes>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log = ::slog::Logger::root(::slog_stdlog::StdLog.fuse(), slog::o!());
    slog_stdlog::init()?;

    let mut event_loop: EventLoop<CallLoopData> = EventLoop::try_new()?;
    let mut display: Display<Wazemmes> = Display::new()?;
    let state = Wazemmes::new(&mut event_loop, &mut display, log.clone());
    let mut data = CallLoopData { state, display };
    winit::init_winit(&mut event_loop, &mut data, log)?;
    std::process::Command::new("alacritty").spawn().ok();
    event_loop.run(None, &mut data, move |_| {
        // Wazemmes is running
    })?;

    Ok(())
}
