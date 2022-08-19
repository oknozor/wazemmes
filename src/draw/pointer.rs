use slog::Logger;

use smithay::backend::renderer::gles2::{Gles2Error, Gles2Frame, Gles2Renderer, Gles2Texture};
use smithay::backend::renderer::{Frame, Texture};
use smithay::desktop::space::{RenderElement, SpaceOutputTuple};

use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Size, Transform};

#[derive(Clone, Debug)]
pub struct PointerElement {
    texture: Gles2Texture,
    position: Point<i32, Logical>,
    size: Size<i32, Logical>,
    damaged: bool,
}

impl PointerElement {
    pub fn new(
        texture: Gles2Texture,
        position: Point<i32, Logical>,
        damaged: bool,
    ) -> PointerElement {
        let size = texture.size().to_logical(1, Transform::Normal);
        PointerElement {
            texture,
            position,
            size,
            damaged,
        }
    }
}

impl RenderElement<Gles2Renderer> for PointerElement {
    fn id(&self) -> usize {
        0
    }

    fn location(&self, scale: impl Into<Scale<f64>>) -> Point<f64, Physical> {
        self.position.to_f64().to_physical(scale)
    }

    fn geometry(&self, scale: impl Into<Scale<f64>>) -> Rectangle<i32, Physical> {
        Rectangle::from_loc_and_size(self.position, self.size).to_physical_precise_round(scale)
    }

    fn accumulated_damage(
        &self,
        scale: impl Into<Scale<f64>>,
        _: Option<SpaceOutputTuple<'_, '_>>,
    ) -> Vec<Rectangle<i32, Physical>> {
        let scale = scale.into();
        if self.damaged {
            vec![Rectangle::from_loc_and_size(self.position, self.size)
                .to_physical_precise_up(scale)]
        } else {
            vec![]
        }
    }

    fn opaque_regions(
        &self,
        _scale: impl Into<Scale<f64>>,
    ) -> Option<Vec<Rectangle<i32, Physical>>> {
        None
    }

    fn draw(
        &self,
        _renderer: &mut Gles2Renderer,
        frame: &mut Gles2Frame,
        scale: impl Into<Scale<f64>>,
        location: Point<f64, Physical>,
        _damage: &[Rectangle<i32, Physical>],
        _dh: &Logger,
    ) -> Result<(), Gles2Error> {
        let scale = scale.into();
        frame.render_texture_at(
            &self.texture,
            location.to_i32_round(),
            1,
            scale,
            Transform::Normal,
            &[Rectangle::from_loc_and_size(
                (0, 0),
                self.size.to_physical_precise_round(scale),
            )],
            1.0,
        )?;
        Ok(())
    }
}
