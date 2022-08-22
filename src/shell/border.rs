use crate::shell::container::ContainerRef;
use crate::shell::window::WindowWrap;
use smithay::desktop::Space;
use smithay::utils::{Logical, Point, Rectangle, Size};

#[derive(Debug, Clone)]
pub struct Border {
    pub left: Rectangle<i32, Logical>,
    pub right: Rectangle<i32, Logical>,
    pub top: Rectangle<i32, Logical>,
    pub bottom: Rectangle<i32, Logical>,
}

pub trait GetBorders {
    fn get_borders(&self, space: &Space) -> Option<Border>;
}

impl GetBorders for WindowWrap {
    fn get_borders(&self, space: &Space) -> Option<Border> {
        space.window_bbox(self.get()).map(|rectangle| {
            let window_loc: Point<i32, Logical> = rectangle.loc;
            let (x, y) = (window_loc.x, window_loc.y);
            let window_size: Size<i32, Logical> = rectangle.size;
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

            Border {
                left,
                right,
                top,
                bottom,
            }
        })
    }
}

impl GetBorders for ContainerRef {
    fn get_borders(&self, _space: &Space) -> Option<Border> {
        let h = self.get().size.h + 4;
        let w = self.get().size.w + 4;
        let x = self.get().location.x - 2;
        let y = self.get().location.y - 2;

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

        Some(Border {
            left,
            right,
            top,
            bottom,
        })
    }
}
