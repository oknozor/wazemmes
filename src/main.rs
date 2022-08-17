#![feature(drain_filter)]
#![feature(hash_drain_filter)]

extern crate core;

use crate::state::{Backend, Wazemmes};
use crate::winit::WinitData;
use slog::Drain;
use smithay::reexports::wayland_server::Display;

mod handlers;
mod inputs;
mod shell;
mod state;
// mod udev;
mod border;
mod config;
mod drawing;
mod render;
mod winit;

pub struct CallLoopData<BackendData: 'static + Backend> {
    state: Wazemmes<BackendData>,
    display: Display<Wazemmes<BackendData>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log = init_log();
    let _guard = slog_scope::set_global_logger(log.clone());

    winit::init_winit(log);

    Ok(())
}

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
