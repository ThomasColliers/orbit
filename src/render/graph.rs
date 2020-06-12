use amethyst::{
    ecs::{
        ReadExpect, SystemData, World,
    },
    renderer::{
        pass::DrawPbrDesc,pass::DrawPbrTransparentDesc,pass::DrawDebugLinesDesc,
        types::DefaultBackend,
        Factory, Format, GraphBuilder, GraphCreator, Kind,
        RenderGroupDesc, SubpassBuilder,
        rendy::graph::render::{SimpleGraphicsPipeline,RenderGroupBuilder},
    },
    ui::{
        DrawUiDesc,
    },
    window::{ScreenDimensions, Window },
};
//use crate::fxaa::DrawFXAADesc;

#[derive(Default)]
pub struct RenderGraph {
    dimensions: Option<ScreenDimensions>,
    dirty: bool,
}

impl GraphCreator<DefaultBackend> for RenderGraph {
    // indicate if it should be rebuilt
    fn rebuild(&mut self, world: &World) -> bool {
        // Rebuild when dimensions change, but wait until at least two frames have the same.
        let new_dimensions = world.try_fetch::<ScreenDimensions>();
        use std::ops::Deref;
        if self.dimensions.as_ref() != new_dimensions.as_deref() {
            self.dirty = true;
            self.dimensions = new_dimensions.map(|d| d.deref().clone());
            return false;
        }
        self.dirty
    }


    // This is the core of a RenderGraph, which is building the actual graph with subpasses and target
    // images.
    fn builder(
        &mut self,
        factory: &mut Factory<DefaultBackend>,
        world: &World,
    ) -> GraphBuilder<DefaultBackend, World> {
        use amethyst::renderer::rendy::{
            graph::present::PresentNode,
            hal::command::{ClearDepthStencil, ClearValue},
        };

        self.dirty = false;

        // Retrieve a reference to the target window, which is created by the WindowBundle
        let window = <ReadExpect<'_, Window>>::fetch(world);
        let dimensions = self.dimensions.as_ref().unwrap();
        let window_kind = Kind::D2(dimensions.width() as u32, dimensions.height() as u32, 1, 1);

        // Create a new drawing surface in our window
        let surface = factory.create_surface(&window);
        let surface_format = factory.get_surface_format(&surface);

        // Begin building our RenderGraph
        let mut graph_builder = GraphBuilder::new();

        // HDR color output
        let hdr = graph_builder.create_image(
            window_kind,
            1,
            Format::Rgba32Sfloat,
            Some(ClearValue::Color([0.0, 0.0, 0.0, 1.0].into())),
        );
        let depth = graph_builder.create_image(
            window_kind,
            1,
            Format::D32Sfloat,
            Some(ClearValue::DepthStencil(ClearDepthStencil(1.0, 0))),
        );

        // Tone mapped output
        let tonemapped = graph_builder.create_image(
            window_kind,
            1,
            Format::Rgba8Unorm, //Format::Rgba8Unorm,
            Some(ClearValue::Color([0.0, 0.0, 0.0, 1.0].into())),
        );

        // Antialiased output
        let antialiased = graph_builder.create_image(
            window_kind,
            1,
            surface_format,
            Some(ClearValue::Color([0.0, 0.0, 0.0, 1.0].into())),
        );

        // Main render pass
        let main_pass = graph_builder.add_node(
            SubpassBuilder::new()
                .with_group(DrawPbrDesc::default().builder())
                .with_group(DrawDebugLinesDesc::new().builder())
                .with_group(DrawPbrTransparentDesc::default().builder())
                .with_group(crate::render::atmosphere::DrawAtmosphereDesc::default().builder())
                .with_group(crate::render::sun::DrawSunDesc::default().builder())
                .with_color(hdr)
                .with_depth_stencil(depth)
                .into_pass(),
        );

        // Post processing pass
        let tonemap_pass = graph_builder.add_node(
            crate::render::tonemap::Pipeline::builder()
                .with_image(hdr)
                .into_subpass()
                .with_dependency(main_pass)
                .with_color(tonemapped)
                .into_pass()
        );

        // FXAA pass
        let fxaa_pass = graph_builder.add_node(
            crate::render::fxaa::Pipeline::builder()
                .with_image(tonemapped)
                .into_subpass()
                .with_dependency(tonemap_pass)
                .with_color(antialiased)
                .into_pass()
        );

        // UI pass
        let ui_pass = graph_builder.add_node(
            SubpassBuilder::new()
                .with_group(DrawUiDesc::default().builder())
                .with_dependency(fxaa_pass)
                .with_color(antialiased)
                .with_depth_stencil(depth)
                .into_pass()
        );

        // Finally, add the pass to the graph
        let _present = graph_builder
            .add_node(PresentNode::builder(factory, surface, antialiased).with_dependency(ui_pass));

        graph_builder
    }
}