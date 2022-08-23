use crate::backend::{NewOutputDescriptor, OutputHandler, OutputId};
use crate::border::{QuadElement, BLUE, RED};
use crate::draw::pointer::PointerElement;
use crate::shell::border::GetBorders;
use crate::state::output::OutputState;
use crate::{BackendState, CallLoopData, Wazemmes};
use smithay::backend::renderer::gles2::{Gles2Renderer, Gles2Texture};
use smithay::desktop::space::SurfaceTree;
use smithay::utils::Transform;
use smithay::wayland::output::{Mode, Output};

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
    ) -> Result<
        Option<Vec<smithay::utils::Rectangle<i32, smithay::utils::Physical>>>,
        smithay::backend::SwapBuffersError,
    > {
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
        let ws = ws.get();
        let (container, window) = ws.get_focus();
        let output_geometry = self.state.space.output_geometry(&output).map(|geometry| {
            let scale = output.current_scale().fractional_scale();
            geometry.to_f64().to_physical_precise_up(scale)
        });

        if let (Some(output_geometry), Some(borders)) =
            (output_geometry, container.get_borders(&self.state.space))
        {
            let transform = self.transform_custom_element();

            renderer
                .with_context(|_renderer, gles| {
                    elems.push(CustomElem::from(QuadElement::new(
                        gles,
                        output_geometry,
                        borders.left,
                        transform,
                        RED,
                    )));

                    elems.push(CustomElem::from(QuadElement::new(
                        gles,
                        output_geometry,
                        borders.top,
                        transform,
                        RED,
                    )));
                    elems.push(CustomElem::from(QuadElement::new(
                        gles,
                        output_geometry,
                        borders.right,
                        transform,
                        RED,
                    )));
                    elems.push(CustomElem::from(QuadElement::new(
                        gles,
                        output_geometry,
                        borders.bottom,
                        transform,
                        RED,
                    )));
                })
                .unwrap()
        }

        if let Some(window) = window {
            let borders = window.get_borders(&self.state.space);

            let output_geometry = self.state.space.output_geometry(&output).map(|geometry| {
                let scale = output.current_scale().fractional_scale();
                geometry.to_f64().to_physical_precise_up(scale)
            });

            if let (Some(output_geometry), Some(borders)) = (output_geometry, borders) {
                let transform = self.transform_custom_element();

                renderer
                    .with_context(|_renderer, gles| {
                        elems.push(CustomElem::from(QuadElement::new(
                            gles,
                            output_geometry,
                            borders.left,
                            transform,
                            BLUE,
                        )));

                        elems.push(CustomElem::from(QuadElement::new(
                            gles,
                            output_geometry,
                            borders.top,
                            transform,
                            BLUE,
                        )));
                        elems.push(CustomElem::from(QuadElement::new(
                            gles,
                            output_geometry,
                            borders.right,
                            transform,
                            BLUE,
                        )));
                        elems.push(CustomElem::from(QuadElement::new(
                            gles,
                            output_geometry,
                            borders.bottom,
                            transform,
                            BLUE,
                        )));
                    })
                    .unwrap()
            }
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
}