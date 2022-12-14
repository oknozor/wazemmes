use crate::backend::{NewOutputDescriptor, OutputHandler, OutputId};
use crate::border::QuadElement;
use crate::draw::pointer::PointerElement;
use crate::shell::drawable::Borders;
use crate::state::output::OutputState;
use crate::{BackendState, CallLoopData, Wazemmes};
use slog_scope::debug;
use smithay::backend::renderer::gles2::{Gles2Renderer, Gles2Texture};
use smithay::desktop::space::SurfaceTree;
use smithay::utils::{Physical, Rectangle, Transform};
use smithay::output::{Mode, Output};

smithay::custom_elements! {
    pub CustomElem<=Gles2Renderer>;
    Quad=QuadElement,
    SurfaceTree=SurfaceTree,
    PointerElement=PointerElement,
}

impl OutputHandler for CallLoopData {
    fn output_created(&mut self, desc: NewOutputDescriptor) {
        let output = Output::new(desc.name.clone(), desc.physical_properties, None);
        output.set_preferred(desc.prefered_mode);

        output.user_data().insert_if_missing(|| desc.id);

        output.create_global::<Wazemmes>(&self.display.handle());

        output.change_current_state(Some(desc.prefered_mode), Some(desc.transform), None, None);

        let outputs: Vec<_> = self
            .state
            .space
            .outputs()
            .cloned()
            .chain(std::iter::once(output))
            .collect();

        let mut x = 0;

        // Put unconfigured outputs on the end
        for output in outputs.into_iter().rev() {
            let location = (x, 0).into();
            self.state.space.map_output(&output, location);
            output.change_current_state(None, None, None, Some(location));

            x += output.current_mode().unwrap().size.w;
        }
    }

    fn output_mode_updated(&mut self, output_id: &OutputId, mode: Mode) {
        let output = self
            .state
            .space
            .outputs()
            .find(|o| o.user_data().get::<OutputId>() == Some(output_id));

        if let Some(output) = output {
            output.change_current_state(Some(mode), None, None, None);
        }
    }

    fn output_render(
        &mut self,
        renderer: &mut Gles2Renderer,
        output_id: &OutputId,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Physical>>>, smithay::backend::SwapBuffersError> {
        let mut elems: Vec<CustomElem> = Vec::new();

        let location = self
            .state
            .seat
            .get_pointer()
            .unwrap()
            .current_location()
            .to_i32_round();

        if let Some(tree) = self.state.pointer_icon.prepare_dnd_icon(location) {
            elems.push(tree.into());
        }

        if let Some(tree) = self.state.pointer_icon.prepare_cursor_icon(location) {
            elems.push(tree.into());
        } else if let Some(texture) = pointer_image {
            elems.push(PointerElement::new(texture.clone(), location, false).into());
        }

        let output = self
            .state
            .space
            .outputs()
            .find(|o| o.user_data().get::<OutputId>() == Some(output_id))
            .unwrap()
            .clone();

        let ws = self.state.get_current_workspace();
        let mut ws = ws.get_mut();

        if let Some(x11) = &mut self.state.x11_state {
            if x11.needs_redraw {
                debug!("X11 Layout update");
                ws.update_layout(&self.state.space);
                x11.needs_redraw = false;
            }
        }

        if ws.needs_redraw {
            debug!("Redraw Workspace({})", self.state.current_workspace);

            ws.redraw(&mut self.state.space, self.state.x11_state.as_mut());

            ws.update_borders();
        }

        let output_geometry = ws.get_output_geometry_f64(&self.state.space);

        if let Some(geometry) = output_geometry {
            self.draw_border(ws.borders.as_slice(), renderer, &mut elems, geometry);
        }

        let output_state = OutputState::for_output(&output);

        let render_result = self
            .state
            .space
            .render_output(renderer, &output, age, [0.1, 0.1, 0.1, 1.0], &elems)
            .unwrap();

        if render_result.is_some() {
            output_state.fps_tick();
        }

        Ok(render_result)
    }

    fn send_frames(&mut self, output_id: &OutputId) {
        let time = self.state.start_time.elapsed().as_millis() as u32;

        // Send frames only to relevant outputs
        for window in self.state.space.windows() {
            let mut output = self.state.space.outputs_for_window(window);

            // Sort by refresh
            output.sort_by_key(|o| o.current_mode().map(|o| o.refresh).unwrap_or(0));
            // Get output with highest refresh
            let best_output_id = output.last().and_then(|o| o.user_data().get::<OutputId>());

            if let Some(best_output_id) = best_output_id {
                if best_output_id == output_id {
                    window.send_frame(self.state.start_time.elapsed().as_millis() as u32);
                }
            } else {
                window.send_frame(time);
            }
        }

        for output in self.state.space.outputs() {
            if output.user_data().get::<OutputId>() == Some(output_id) {
                let map = smithay::desktop::layer_map_for_output(output);
                for layer in map.layers() {
                    layer.send_frame(time);
                }
            }
        }
    }
}

impl CallLoopData {
    // QuadElement needs to be flipped when running via udev
    fn transform_custom_element(&self) -> Transform {
        match self.state.backend {
            BackendState::Drm(_) => Transform::Flipped180,
            BackendState::None => Transform::Normal,
        }
    }

    fn draw_border(
        &mut self,
        borders: &[Borders],
        renderer: &mut Gles2Renderer,
        elems: &mut Vec<CustomElem>,
        geometry: Rectangle<f64, Physical>,
    ) {
        let transform = self.transform_custom_element();
        for border in borders {
            renderer
                .with_context(|_renderer, gles| {
                    let borders = QuadElement::new(gles, geometry, border, transform)
                        .into_iter()
                        .map(CustomElem::from);
                    elems.extend(borders);
                })
                .expect("Failed to render borders")
        }
    }
}
