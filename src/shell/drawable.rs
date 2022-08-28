use crate::border::{BLUE, RED};
use crate::shell::container::Container;
use crate::shell::node::Node;
use crate::shell::windows::toplevel::WindowWrap;
use smithay::utils::{Logical, Point, Rectangle, Size};

pub trait Border {
    fn make_borders(&self) -> Borders;
}

#[derive(Debug, Clone)]
pub struct Borders {
    pub(crate) color: (f32, f32, f32),
    pub left: Rectangle<i32, Logical>,
    pub right: Rectangle<i32, Logical>,
    pub top: Rectangle<i32, Logical>,
    pub bottom: Rectangle<i32, Logical>,
}

impl Default for Borders {
    fn default() -> Self {
        Self {
            color: (0.0, 0.0, 0.0),
            left: Default::default(),
            right: Default::default(),
            top: Default::default(),
            bottom: Default::default(),
        }
    }
}

impl Border for WindowWrap {
    fn make_borders(&self) -> Borders {
        let window_size = self.size();
        let window_loc = self.location();
        let (x, y) = (window_loc.x, window_loc.y);
        let (w, h) = (window_size.w, window_size.h);

        let left = {
            let topleft = (x - 2, y - 2);
            let bottom_right = (x, y + h);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let top = {
            let topleft = (x, y - 2);
            let bottom_right = (x + w + 2, y);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let bottom = {
            let topleft = (x - 2, y + h);
            let bottom_right = (x + w + 2, y + h + 2);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let right = {
            let topleft = (x + w, y);
            let bottom_right = (x + w + 2, y + h + 2);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        Borders {
            color: BLUE,
            left,
            right,
            top,
            bottom,
        }
    }
}

impl Border for Container {
    fn make_borders(&self) -> Borders {
        let h = self.size.h + 4;
        let w = self.size.w + 4;
        let x = self.location.x - 2;
        let y = self.location.y - 2;

        let left = {
            let topleft = (x - 2, y - 2);
            let bottom_right = (x, y + h);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let top = {
            let topleft = (x, y - 2);
            let bottom_right = (x + w + 2, y);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let bottom = {
            let topleft = (x - 2, y + h);
            let bottom_right = (x + w + 2, y + h + 2);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let right = {
            let topleft = (x + w, y);
            let bottom_right = (x + w + 2, y + h + 2);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        Borders {
            color: RED,
            left,
            right,
            top,
            bottom,
        }
    }
}
