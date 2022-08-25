use crate::shell::container::ContainerRef;
use crate::shell::window::WindowWrap;

#[derive(Debug, Clone)]
pub enum Node {
    Container(ContainerRef),
    Window(WindowWrap),
}

impl Node {
    pub fn is_container(&self) -> bool {
        matches!(self, Node::Container(_))
    }

    pub fn id(&self) -> u32 {
        match self {
            Node::Container(container) => container.get().id,
            Node::Window(w) => w.id(),
        }
    }
}

impl TryInto<WindowWrap> for Node {
    // TODO: this error
    type Error = &'static str;

    fn try_into(self) -> Result<WindowWrap, Self::Error> {
        match self {
            Node::Container(_) => Err("tried to unwrap a window got a container"),
            Node::Window(w) => Ok(w),
        }
    }
}

impl<'a> TryInto<&'a mut WindowWrap> for &'a mut Node {
    // TODO: this error
    type Error = &'static str;

    fn try_into(self) -> Result<&'a mut WindowWrap, Self::Error> {
        match self {
            Node::Container(_) => Err("tried to unwrap a window got a container"),
            Node::Window(w) => Ok(w),
        }
    }
}

impl TryInto<WindowWrap> for &Node {
    // TODO: this error
    type Error = &'static str;

    fn try_into(self) -> Result<WindowWrap, Self::Error> {
        match self {
            Node::Container(_) => Err("tried to unwrap a window got a container"),
            Node::Window(w) => Ok(w.clone()),
        }
    }
}

impl TryInto<ContainerRef> for Node {
    // TODO: this error
    type Error = &'static str;

    fn try_into(self) -> Result<ContainerRef, Self::Error> {
        match self {
            Node::Container(c) => Ok(c),
            Node::Window(_) => Err("tried to unwrap a container got a window"),
        }
    }
}

impl TryInto<ContainerRef> for &Node {
    // TODO: this error
    type Error = &'static str;

    fn try_into(self) -> Result<ContainerRef, Self::Error> {
        match self {
            Node::Container(c) => Ok(c.clone()),
            Node::Window(_) => Err("tried to unwrap a container got a window"),
        }
    }
}

impl<'a> TryInto<&'a WindowWrap> for &'a Node {
    // TODO: this error
    type Error = &'static str;

    fn try_into(self) -> Result<&'a WindowWrap, Self::Error> {
        match self {
            Node::Container(_) => Err("tried to unwrap a window got a container"),
            Node::Window(w) => Ok(w),
        }
    }
}

impl<'a> TryInto<&'a ContainerRef> for &'a Node {
    // TODO: this error
    type Error = &'static str;

    fn try_into(self) -> Result<&'a ContainerRef, Self::Error> {
        match self {
            Node::Container(c) => Ok(c),
            Node::Window(_) => Err("tried to unwrap a container got a window"),
        }
    }
}

pub mod id {
    use once_cell::sync::Lazy;
    use std::sync::{Arc, Mutex};

    static NODE_ID_COUNTER: Lazy<Arc<Mutex<u32>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

    pub fn get() -> u32 {
        let id = NODE_ID_COUNTER.lock().unwrap();
        *id
    }

    pub fn next() -> u32 {
        let mut id = NODE_ID_COUNTER.lock().unwrap();
        *id += 1;
        *id
    }
}
