// Custom `RenderPlugin` to render the atmosphere to be used with `RenderingBundle`

use amethyst::renderer::{
    bundle::{Target, RenderOrder, RenderPlan, RenderPlugin},
    Backend, Factory, Mesh,
    submodules::{DynamicVertexBuffer, EnvironmentSub },
    ChangeDetection,
    pod::VertexArgs,
    pipeline::{PipelineDescBuilder, PipelinesBuilder},
    util,
    visibility::Visibility,
    batch::{GroupIterator, OrderedOneLevelBatch},
};
use amethyst::core::{
    ecs::{Join, Read, ReadExpect, ReadStorage, SystemData, World},
    transform::Transform,
};
use amethyst::assets::{AssetStorage, Handle};
use amethyst::error::Error;
use derivative::Derivative;
use rendy::{
    command::{QueueId, RenderPassEncoder},
    hal::{self, device::Device, pso, pso::ShaderStageFlags},
    graph::{
        render::{PrepareResult, RenderGroup, RenderGroupDesc},
        GraphContext, NodeBuffer, NodeImage,
    },
    mesh::{
        VertexFormat, TexCoord, Tangent, Position, Normal, AsVertex
    },
    shader::{Shader, SpirvShader},
};

// plugin
#[derive(Default, Debug)]
pub struct RenderAtmosphere {
    target: Target,
}

impl<B: Backend> RenderPlugin<B> for RenderAtmosphere {
    fn on_plan(
        &mut self,
        plan: &mut RenderPlan<B>,
        _factory: &mut Factory<B>,
        _world: &World
    ) -> Result<(), Error> {
        plan.extend_target(self.target, |ctx| {
            ctx.add(
                RenderOrder::Transparent,
                DrawAtmosphereDesc::new().builder(),
            )?;
            Ok(())
        });
        Ok(())
    }
}

// load our shader
lazy_static::lazy_static! {
    static ref VERTEX:SpirvShader = SpirvShader::from_bytes(
        include_bytes!("../../assets/shader/atmosphere.vert.spv"),
        ShaderStageFlags::VERTEX,
        "main",
    ).unwrap();

    static ref FRAGMENT:SpirvShader = SpirvShader::from_bytes(
        include_bytes!("../../assets/shader/atmosphere.frag.spv"),
        ShaderStageFlags::VERTEX,
        "main",
    ).unwrap();
}

// plugin desc
#[derive(Clone, PartialEq, Derivative)]
#[derivative(Debug(bound = ""), Default(bound = ""))]
pub struct DrawAtmosphereDesc;

impl DrawAtmosphereDesc {
    pub fn new() -> Self {
        Default::default()
    }
}

fn build_custom_pipeline<B: Backend>(
    factory: &Factory<B>,
    subpass: hal::pass::Subpass<'_, B>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    vertex_format: &[VertexFormat],
    layouts: Vec<&B::DescriptorSetLayout>,
) -> Result<(B::GraphicsPipeline, B::PipelineLayout), failure::Error> {
    let pipeline_layout = unsafe {
        factory
            .device()
            .create_pipeline_layout(layouts, None as Option<(_, _)>)
    }?;

    // vertex descriptor
    let vertex_desc = vertex_format
        .iter()
        .map(|f| (f.clone(), pso::VertexInputRate::Vertex))
        .chain(Some((
            VertexArgs::vertex(),
            pso::VertexInputRate::Instance(1)
        )))
        .collect::<Vec<_>>();

    // get shaders
    let shader_vertex = unsafe { VERTEX.module(factory).unwrap() };
    let shader_fragment = unsafe { FRAGMENT.module(factory).unwrap() };

    // build the pipeline
    let pipes = PipelinesBuilder::new()
        .with_pipeline(
            PipelineDescBuilder::new()
                .with_vertex_desc(&vertex_desc)
                .with_shaders(util::simple_shader_set(
                    &shader_vertex,
                    Some(&shader_fragment),
                ))
                .with_layout(&pipeline_layout)
                .with_subpass(subpass)
                .with_framebuffer_size(framebuffer_width, framebuffer_height)
                .with_face_culling(pso::Face::BACK)
                .with_depth_test(pso::DepthTest {
                    fun: pso::Comparison::Less,
                    write: false, // as our shader will be transparent
                })
                // alpha blended
                .with_blend_targets(vec![pso::ColorBlendDesc {
                    mask: pso::ColorMask::ALL,
                    blend: Some(pso::BlendState::PREMULTIPLIED_ALPHA),
                }])
        )
        .build(factory, None);
    
    // destroy the shaders when loaded
    unsafe {
        factory.destroy_shader_module(shader_vertex);
        factory.destroy_shader_module(shader_fragment);
    }

    // handle errors and return
    match pipes {
        Err(e) => {
            unsafe {
                factory.device().destroy_pipeline_layout(pipeline_layout);
            }
            Err(e)
        }
        Ok(mut pipes) => Ok((pipes.remove(0), pipeline_layout)),
    }
}

impl<B: Backend> RenderGroupDesc<B, World> for DrawAtmosphereDesc {
    fn build(
        self,
        _ctx: &GraphContext<B>,
        factory: &mut Factory<B>,
        _queue: QueueId,
        _aux: &World,
        framebuffer_width: u32,
        framebuffer_height: u32,
        subpass: hal::pass::Subpass<'_, B>,
        _buffers: Vec<NodeBuffer>,
        _images: Vec<NodeImage>,
    ) -> Result<Box<dyn RenderGroup<B, World>>, failure::Error> {
        let env = EnvironmentSub::new(
            factory,
            [
                ShaderStageFlags::VERTEX,
                ShaderStageFlags::FRAGMENT,
            ],
        )?;

        let mut vertex_format = vec![
            Position::vertex(),
            Normal::vertex(),
            Tangent::vertex(),
            TexCoord::vertex(),
        ];

        let (pipeline, pipeline_layout) = build_custom_pipeline(
            factory,
            subpass,
            framebuffer_width,
            framebuffer_height,
            &vertex_format,
            vec![env.raw_layout()],
        )?;

        // not sure if/why this is needed but this is done in base_3d as well
        vertex_format.sort();

        Ok(Box::new(DrawAtmosphere::<B> {
            pipeline: pipeline,
            pipeline_layout: pipeline_layout,
            env: env,
            batches: Default::default(),
            vertex_format: vertex_format,
            models: DynamicVertexBuffer::new(),
            change: Default::default(),
        }))
    }
}

// implementation of the render pass
#[derive(Debug)]
pub struct DrawAtmosphere<B: Backend> {
    pipeline: B::GraphicsPipeline,
    pipeline_layout: B::PipelineLayout,
    env: EnvironmentSub<B>,
    batches: OrderedOneLevelBatch<u32, VertexArgs>,
    vertex_format: Vec<VertexFormat>,
    models: DynamicVertexBuffer<B, VertexArgs>,
    change: ChangeDetection
}

impl<B: Backend> RenderGroup<B, World> for DrawAtmosphere<B> {
    fn prepare(
        &mut self,
        factory: &Factory<B>,
        _queue: QueueId,
        index: usize,
        _subpass: hal::pass::Subpass<B>,
        world: &World,
    ) -> PrepareResult {
        // get components from the ecs
        let (
            mesh_storage,
            visibility,
            meshes,
            transforms,
        ) = <(
            Read<'_, AssetStorage<Mesh>>,
            ReadExpect<'_, Visibility>,
            ReadStorage<'_, Handle<Mesh>>,
            ReadStorage<'_, Transform>,
        )>::fetch(world);

        // prepare environemnt
        self.env.process(factory, index, world);

        // clear batches
        self.batches.swap_clear();

        // refs
        let batches_ref = &mut self.batches;
        let mut changed = false;

        // setup the batches
        let mut joined = (&meshes, &transforms).join();
        visibility
            .visible_ordered
            .iter()
            .filter_map(|e| joined.get_unchecked(e.id()))
            .map(|(mesh, tform)| {
                ((mesh.id()),VertexArgs::from_object_data(tform, None))
            })
            .for_each_group(|mesh_id, data| {
                if mesh_storage.contains_id(mesh_id) {
                    batches_ref.insert(mesh_id, data.drain(..));
                }
            });
        
        // write models
        self.models.write(
            factory,
            index,
            self.batches.count() as u64,
            Some(self.batches.data()),
        );

        // update changed status
        changed = changed || self.batches.changed();

        self.change.prepare_result(index, changed)
    }

    fn draw_inline(
        &mut self,
        mut encoder: RenderPassEncoder<'_, B>,
        index: usize,
        _subpass: hal::pass::Subpass<'_, B>,
        resources: &World,
    ) {
        let mesh_storage = <Read<'_, AssetStorage<Mesh>>>::fetch(resources);
        let layout = &self.pipeline_layout;
        let encoder = &mut encoder;

        let models_loc = self.vertex_format.len() as u32;

        encoder.bind_graphics_pipeline(&self.pipeline);
        self.env.bind(index, layout, 0, encoder);

        if self.models.bind(index, models_loc, 0, encoder) {
            for (mesh, range) in self.batches.iter() {
                if let Some(mesh) =
                    B::unwrap_mesh(unsafe { mesh_storage.get_by_id_unchecked(*mesh) })
                {
                    if let Err(error) = mesh.bind_and_draw(
                        0,
                        &self.vertex_format,
                        range.clone(),
                        encoder,
                    ) {
                        log::warn!(
                            "Trying to draw a mesh that lacks {:?} vertex attributes. Pass {} requires attributes {:?}.",
                            error.not_found.attributes,
                            "Atmosphere",
                            &self.vertex_format,
                        );
                    }
                }
            }
        }
    }

    fn dispose(self: Box<Self>, factory: &mut Factory<B>, _aux: &World) {
        unsafe {
            factory.device().destroy_graphics_pipeline(self.pipeline);
            factory.device().destroy_pipeline_layout(self.pipeline_layout);
        }
    }
}