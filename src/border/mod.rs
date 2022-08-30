#![allow(dead_code)]

use std::num::NonZeroU32;

use cgmath::Matrix3;
use slog::Logger;
use smithay::backend::renderer::gles2::ffi::{self, Gles2};
use smithay::backend::renderer::gles2::Gles2Renderer;
use smithay::backend::renderer::Renderer;
use smithay::desktop::space::{RenderElement, SpaceOutputTuple};

use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Transform};

mod glow;

use crate::shell::drawable::Borders;
use glow::{Program, Shader};
use crate::backend::drawing::BORDER_Z_INDEX;

pub const BLUE: (f32, f32, f32) = (26.0 / 255.0, 95.0 / 255.0, 205.0 / 255.0);
pub const RED: (f32, f32, f32) = (1.0, 95.0 / 255.0, 205.0 / 255.0);

pub struct QuadPipeline {
    program: glow::Program,
    projection: glow::UniformLocation,
    gl_color: glow::UniformLocation,
    color: (f32, f32, f32),
    position: u32, // AtributeLocation,
}

impl QuadPipeline {
    pub fn new(gl: &Gles2, border_color: (f32, f32, f32)) -> Self {
        let program = create_program(
            gl,
            include_str!("./shaders/quad.vert"),
            include_str!("./shaders/quad.frag"),
        );

        let (projection, color, position) = unsafe {
            (
                glow::get_uniform_location(gl, program, "projection").unwrap(),
                glow::get_uniform_location(gl, program, "color").unwrap(),
                glow::get_attrib_location(gl, program, "position").unwrap(),
            )
        };

        Self {
            program,
            projection,
            position,
            gl_color: color,
            color: border_color,
        }
    }

    pub fn render(
        &self,
        output_geometry: Rectangle<f64, Physical>,
        mut quad_rect: Rectangle<f64, Physical>,
        transform: Transform,
        gl: &Gles2,
        alpha: f32,
    ) {
        quad_rect.loc.x -= output_geometry.loc.x;

        let screen = Matrix3 {
            x: [2.0 / output_geometry.size.w as f32, 0.0, 0.0].into(),
            y: [0.0, -2.0 / output_geometry.size.h as f32, 0.0].into(),
            z: [-1.0, 1.0, 1.0].into(),
        };


        let x = quad_rect.loc.x as f32;
        let y = quad_rect.loc.y as f32;

        let w = quad_rect.size.w as f32;
        let h = quad_rect.size.h as f32;

        let quad = Matrix3 {
            x: [w, 0.0, 0.0].into(),
            y: [0.0, h, 0.0].into(),
            z: [x, y, 1.0].into(),
        };

        unsafe {
            gl.UseProgram(self.program.0.into());

            let mat = transform.matrix() * screen * quad;
            let mat: &[f32; 9] = mat.as_ref();

            gl.UniformMatrix3fv(
                self.projection.0 as i32,
                mat.len() as i32 / 9,
                false as u8,
                mat.as_ptr(),
            );

            gl.Uniform4f(
                self.gl_color.0 as i32,
                self.color.0,
                self.color.1,
                self.color.2,
                alpha,
            );

            gl.VertexAttribPointer(
                self.position,
                2,
                ffi::FLOAT,
                ffi::FALSE as u8,
                0,
                VERTS.as_ptr() as *const _,
            );

            gl.EnableVertexAttribArray(self.position);

            gl.DrawArrays(ffi::TRIANGLE_STRIP, 0, 4);

            gl.DisableVertexAttribArray(self.position);
            gl.UseProgram(0);
        }
    }
}

pub struct QuadElement {
    transform: Transform,
    pipeline: QuadPipeline,
    geometry: Rectangle<i32, Logical>,
    output_geometry: Rectangle<f64, Physical>,
}

impl QuadElement {
    pub fn new(
        gl: &Gles2,
        output_geometry: Rectangle<f64, Physical>,
        border: &Borders,
        transform: Transform,
    ) -> [Self; 4] {
        [
            Self {
                transform,
                pipeline: QuadPipeline::new(gl, border.color),
                geometry: border.left,
                output_geometry,
            },
            Self {
                transform,
                pipeline: QuadPipeline::new(gl, border.color),
                geometry: border.right,
                output_geometry,
            },
            Self {
                transform,
                pipeline: QuadPipeline::new(gl, border.color),
                geometry: border.top,
                output_geometry,
            },
            Self {
                transform,
                pipeline: QuadPipeline::new(gl, border.color),
                geometry: border.bottom,
                output_geometry,
            },
        ]
    }
}

impl RenderElement<Gles2Renderer> for QuadElement {
    fn id(&self) -> usize {
        // Fixme: correctly handle negative values
        let hash = format!(
            "{}{}{}{}",
            self.geometry.loc.x.abs(),
            self.geometry.loc.y.abs(),
            self.geometry.size.w.abs(),
            self.geometry.size.h.abs()
        );

        hash.parse::<usize>().unwrap()
    }

    fn location(&self, scale: impl Into<Scale<f64>>) -> Point<f64, Physical> {
        self.geometry.loc.to_f64().to_physical(scale)
    }

    fn geometry(&self, scale: impl Into<Scale<f64>>) -> Rectangle<i32, Physical> {
        Rectangle::from_loc_and_size(self.geometry.loc, self.geometry.size)
            .to_physical_precise_round(scale)
    }

    fn accumulated_damage(
        &self,
        scale: impl Into<Scale<f64>>,
        _: Option<SpaceOutputTuple<'_, '_>>,
    ) -> Vec<Rectangle<i32, Physical>> {
        vec![self.geometry.to_physical_precise_round(scale)]
    }

    fn opaque_regions(
        &self,
        _scale: impl Into<Scale<f64>>,
    ) -> Option<Vec<Rectangle<i32, Physical>>> {
        None
    }

    // TODO: Make sure this is not rendered on every frame
    fn draw(
        &self,
        renderer: &mut Gles2Renderer,
        _frame: &mut <Gles2Renderer as Renderer>::Frame,
        scale: impl Into<Scale<f64>>,
        location: Point<f64, Physical>,
        _damage: &[Rectangle<i32, Physical>],
        _log: &Logger,
    ) -> Result<(), <Gles2Renderer as Renderer>::Error> {
        renderer.with_context(|_, gl| {
            self.pipeline.render(
                self.output_geometry,
                Rectangle::from_loc_and_size(
                    self.output_geometry.loc.to_f64() + location,
                    self.geometry.size.to_f64().to_physical(scale),
                ),
                self.transform,
                gl,
                1.0,
            )
        })
    }

    fn z_index(&self) -> u8 {
        BORDER_Z_INDEX
    }
}

static VERTS: [ffi::types::GLfloat; 8] = [
    1.0, 0.0, // top right
    0.0, 0.0, // top left
    1.0, 1.0, // bottom right
    0.0, 1.0, // bottom left
];

fn create_program(gl: &Gles2, vertex_shader_source: &str, fragment_shader_source: &str) -> Program {
    unsafe {
        let program = gl.CreateProgram();
        let program = Program(NonZeroU32::new(program).unwrap());

        let shader_sources = [
            (ffi::VERTEX_SHADER, vertex_shader_source),
            (ffi::FRAGMENT_SHADER, fragment_shader_source),
        ];

        let mut shaders = Vec::with_capacity(shader_sources.len());

        for (shader_type, shader_source) in shader_sources.iter() {
            let shader = gl.CreateShader(*shader_type);
            let shader = Shader(NonZeroU32::new(shader).unwrap());

            gl.ShaderSource(
                shader.0.into(),
                1,
                &(shader_source.as_ptr() as *const ffi::types::GLchar),
                &(shader_source.len() as ffi::types::GLint),
            );

            gl.CompileShader(shader.0.into());

            if !glow::get_shader_compile_status(gl, shader) {
                panic!("{}", glow::get_shader_info_log(gl, shader));
            }
            gl.AttachShader(program.0.into(), shader.0.into());
            shaders.push(shader);
        }

        gl.LinkProgram(program.0.into());
        if !glow::get_program_link_status(gl, program) {
            panic!("{}", glow::get_program_info_log(gl, program));
        }

        for shader in shaders {
            gl.DetachShader(program.0.into(), shader.0.into());
            gl.DeleteShader(shader.0.into());
        }

        program
    }
}
