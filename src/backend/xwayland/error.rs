use thiserror::Error;
#[derive(Error, Debug)]
pub enum XWaylandError {
    #[error("X11 Client error")]
    ConnectionError(#[from] x11rb::errors::ConnectionError),
    #[error("X11 Client reply error")]
    ReplyError(#[from] x11rb::errors::ReplyError),
    #[error("X11 Atom({0}) is not supported")]
    UnknownAtom(u32),
    #[error("Empty reply")]
    EmptyReply


}