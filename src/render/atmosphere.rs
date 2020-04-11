// Custom `RenderPlugin` to render the atmosphere to be used with `RenderingBundle`

use amethyst::renderer::{
    bundle::{Target, RenderOrder, RenderPlan, RenderPlugin},
    Backend, Factory, Mesh,
    submodules::{DynamicVertexBuffer, EnvironmentSub, DynamicUniform },
    ChangeDetection,
    pod::VertexArgs,
    pipeline::{PipelineDescBuilder, PipelinesBuilder},
    util,
    visibility::Visibility,
};
use amethyst::core::{
    ecs::{Join, Read, ReadExpect, ReadStorage, SystemData, World},
    transform::Transform,
    Hidden, HiddenPropagate,
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

impl RenderAtmosphere {
    pub fn with_target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }
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
#[derive(Debug, Clone, PartialEq, Derivative)]
#[derivative(Default(bound = ""))]
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
    layouts: Vec<&B::DescriptorSetLayout>,
) -> Result<(B::GraphicsPipeline, B::PipelineLayout), failure::Error> {
    let pipeline_layout = unsafe {
        factory
            .device()
            .create_pipeline_layout(layouts, None as Option<(_, _)>)
    }?;

    // get shaders
    let shader_vertex = unsafe { VERTEX.module(factory).unwrap() };
    let shader_fragment = unsafe { FRAGMENT.module(factory).unwrap() };

    let vertex_format = VertexFormat::new((
        Position::vertex(),
        Normal::vertex(),
        Tangent::vertex(),
        TexCoord::vertex(),
    ));

    // build the pipeline
    let pipes = PipelinesBuilder::new()
        .with_pipeline(
            PipelineDescBuilder::new()
                .with_vertex_desc(&[(vertex_format, pso::VertexInputRate::Vertex)])
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
                    blend: Some(pso::BlendState::ALPHA),
                }]),
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
        //let vertex = DynamicVertexBuffer::new();

        let (pipeline, pipeline_layout) = build_custom_pipeline(
            factory,
            subpass,
            framebuffer_width,
            framebuffer_height,
            vec![env.raw_layout()],
        )?;

        Ok(Box::new(DrawAtmosphere::<B> {
            pipeline: pipeline,
            pipeline_layout: pipeline_layout,
            env: env,
            meshes: Vec::new(),
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
    meshes:Vec<u32>,
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
            hiddens,
            hiddens_prop,
            meshes,
            transforms,
        ) = <(
            Read<'_, AssetStorage<Mesh>>,
            ReadExpect<'_, Visibility>,
            ReadStorage<'_, Hidden>,
            ReadStorage<'_, HiddenPropagate>,
            ReadStorage<'_, Handle<Mesh>>,
            ReadStorage<'_, Transform>,
        )>::fetch(world);

        // prepare environemnt
        self.env.process(factory, index, world);

        // keep track if there are any changes
        let mut changed = false;

        // prepare references to meshes for drawing
        let mesh_count:usize = 0;
        let mut mesh_vec = Vec::new();
        for (mesh, transform) in (&meshes, &transforms).join() {
            let id = mesh.id();
            if mesh_storage.contains_id(id){
                mesh_vec.push(id);
            }
        };
        if mesh_vec.len() != self.meshes.len() {
            self.meshes = mesh_vec;
            changed = true;
        }

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

        encoder.bind_graphics_pipeline(&self.pipeline);
        self.env.bind(index, layout, 0, encoder);

        for mesh_id in &self.meshes {
            if let Some(mesh) = B::unwrap_mesh(unsafe { mesh_storage.get_by_id_unchecked(*mesh_id) }) {
                println!("draw mesh");
                let vertex_format = VertexFormat::new((
                    Position::vertex(),
                    Normal::vertex(),
                    Tangent::vertex(),
                    TexCoord::vertex(),
                ));
                if let Err(error) = mesh.bind_and_draw(
                    0,
                    &[vertex_format],
                    0..100,
                    encoder,
                ) {
                    /*log::warn!(
                        "Trying to draw a mesh that lacks {:?} vertex attributes. Pass {} requires attributes {:?}.",
                        error.not_found.attributes,
                        T::NAME,
                        T::base_format(),
                    );*/
                }
            }
        }

    }

    fn dispose(mut self: Box<Self>, factory: &mut Factory<B>, _aux: &World) {
        unsafe {
            factory.device().destroy_graphics_pipeline(self.pipeline);
            factory.device().destroy_pipeline_layout(self.pipeline_layout);
        }
    }
}