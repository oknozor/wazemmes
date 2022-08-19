use crate::backend::{NewOutputDescriptor, OutputHandler, OutputId};
use crate::border::QuadElement;
use crate::draw::pointer::PointerElement;
use crate::state::output::OutputState;
use crate::{CallLoopData, Wazemmes};
use slog_scope::debug;
use smithay::backend::renderer::gles2::{Gles2Renderer, Gles2Texture};
use smithay::desktop::space::SurfaceTree;
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

        // if let Some(c) = layout.find_output(&desc.name) {
        //     output.change_current_state(Some(c.mode()), Some(desc.transform), None, None);
        //     self.backend
        //         .update_mode(output.user_data().get::<OutputId>().unwrap(), &c.mode());
        // } else {
        //     output.change_current_state(Some(desc.prefered_mode), Some(desc.transform), None, None);
        // }

        let outputs: Vec<_> = self
            .state
            .space
            .outputs()
            .cloned()
            .chain(std::iter::once(output))
            .collect();

        let mut x = 0;

        // // Map all configured outputs first
        // for desc in layout.iter() {
        //     if let Some(id) = outputs.iter().position(|o| o.name() == desc.name()) {
        //         let output = outputs.remove(id);

        //         let location = (x, 0).into();
        //         self.space.map_output(&output, 1.0, location);

        //         output.change_current_state(None, None, None, Some(location));

        //         x += desc.mode().size.w;
        //     }
        // }

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
            debug!("preparing dnd icon");
            elems.push(tree.into());
        }

        if let Some(tree) = self.state.pointer_icon.prepare_cursor_icon(location) {
            debug!("preparing cursor icon");
            elems.push(tree.into());
        } else if let Some(texture) = pointer_image {
            debug!("preparing pointer image icon");
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
        let (_, window) = ws.get_focus();
        if let Some(window) = window {
            // TODO: get the correct output, not just the first one
            let borders = window.get_borders(&self.state.space);

            let output_geometry = self.state.space.output_geometry(&output).map(|geometry| {
                let scale = output.current_scale().fractional_scale();
                geometry.to_f64().to_physical_precise_up(scale)
            });

            if let (Some(output_geometry), Some(borders)) = (output_geometry, borders) {
                renderer
                    .with_context(|_renderer, gles| {
                        elems.push(CustomElem::from(QuadElement::new(
                            gles,
                            output_geometry,
                            borders.left,
                        )));
                        elems.push(CustomElem::from(QuadElement::new(
                            gles,
                            output_geometry,
                            borders.top,
                        )));
                        elems.push(CustomElem::from(QuadElement::new(
                            gles,
                            output_geometry,
                            borders.right,
                        )));
                        elems.push(CustomElem::from(QuadElement::new(
                            gles,
                            output_geometry,
                            borders.bottom,
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
}
