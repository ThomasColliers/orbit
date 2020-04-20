// Custom `RenderPlugin` to render fxaa post-processing to be used with `RenderingBundle`

use amethyst::renderer::{
    bundle::{Target, TargetImage, RenderOrder, RenderPlan, RenderPlugin},
    Backend, Factory,
    submodules::{DynamicUniform},
    pipeline::{PipelineDescBuilder, PipelinesBuilder},
    util,
};
use amethyst::{
    ecs::{World},
    error::Error,
    prelude::*,
    window::ScreenDimensions,
};
use rendy::{
    command::{QueueId, RenderPassEncoder},
    hal::{self, device::Device, pso, pso::ShaderStageFlags, format::Format},
    graph::{
        render::{PrepareResult, RenderGroup, RenderGroupDesc},
        GraphContext, NodeBuffer, NodeImage, ImageId
    },
    mesh::{
        VertexFormat, AsVertex
    },
    shader::{Shader, SpirvShader},
    memory,
    resource::{self,Escape,BufferInfo,Buffer,DescriptorSet,Handle as RendyHandle,DescriptorSetLayout,ImageView,ImageViewInfo},
};
use glsl_layout::*;

// plugin
#[derive(Default, Debug)]
pub struct RenderFXAA {
    target: Target,
}

impl RenderFXAA {
    pub fn with_target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }
}

impl<B: Backend> RenderPlugin<B> for RenderFXAA {
    fn on_plan(
        &mut self,
        plan: &mut RenderPlan<B>,
        _factory: &mut Factory<B>,
        _world: &World
    ) -> Result<(), Error> {
        plan.extend_target(self.target, |ctx| {
            let source_image = TargetImage::Color(Target::Custom("offscreen"), 0);
            let source_id = ctx.get_image(source_image).unwrap();
            ctx.add(
                RenderOrder::Opaque,
                DrawFXAADesc::new(source_id).builder(),
            )?;
            Ok(())
        });
        Ok(())
    }
}

// load our shader pair
lazy_static::lazy_static! {
    static ref VERTEX:SpirvShader = SpirvShader::from_bytes(
        include_bytes!("../../assets/shader/fxaa.vert.spv"),
        ShaderStageFlags::VERTEX,
        "main",
    ).unwrap();

    static ref FRAGMENT:SpirvShader = SpirvShader::from_bytes(
        include_bytes!("../../assets/shader/fxaa.frag.spv"),
        ShaderStageFlags::FRAGMENT,
        "main",
    ).unwrap();
}

// plugin desc
#[derive(Clone, PartialEq, Debug)]
pub struct DrawFXAADesc {
    source: ImageId,
}

impl DrawFXAADesc {
    pub fn new(source: ImageId) -> Self {
        Self {
            source: source
        }
    }
}

impl<B: Backend> RenderGroupDesc<B, World> for DrawFXAADesc {
    fn build(
        self,
        ctx: &GraphContext<B>,
        factory: &mut Factory<B>,
        _queue: QueueId,
        _aux: &World,
        framebuffer_width: u32,
        framebuffer_height: u32,
        subpass: hal::pass::Subpass<'_, B>,
        _buffers: Vec<NodeBuffer>,
        _images: Vec<NodeImage>,
    ) -> Result<Box<dyn RenderGroup<B, World>>, failure::Error> {
        // this will keep our screen dimensions uniforms
        let env = DynamicUniform::new(factory, pso::ShaderStageFlags::FRAGMENT)?;

        // get view on offscreen image
        let image = ctx.get_image(self.source).unwrap();
        let view = ImageView::create(
            factory.device(),
            ImageViewInfo {
                view_kind:resource::ViewKind::D2,
                format:hal::format::Format::Rgb8Unorm,
                swizzle:hal::format::Swizzle::NO,
                range:resource::SubresourceRange {
                    aspects:hal::format::Aspects::COLOR,
                    levels:0..1,
                    layers:0..1,
                }
            },
            image.clone(),
        ).unwrap();

        // setup the offscreen texture descriptor set
        let texture_layout:RendyHandle<DescriptorSetLayout<B>> = RendyHandle::from(
            factory
            .create_descriptor_set_layout(vec![hal::pso::DescriptorSetLayoutBinding {
                binding: 0,
                ty: pso::DescriptorType::CombinedImageSampler,
                count: 1,
                stage_flags: pso::ShaderStageFlags::FRAGMENT,
                immutable_samplers: false,
            }])
            .unwrap()
        );
        let texture_set = factory.create_descriptor_set(texture_layout.clone()).unwrap();

        // write to the texture description set
        unsafe {
            factory.device().write_descriptor_sets(vec![
                hal::pso::DescriptorSetWrite {
                    set: texture_set.raw(),
                    binding: 0,
                    array_offset: 0,
                    descriptors: vec![pso::Descriptor::Image(
                        view.raw(),
                        hal::image::Layout::ShaderReadOnlyOptimal
                    )]
                }
            ]);
        }

        // setup the pipeline
        let (pipeline, pipeline_layout) = build_custom_pipeline(
            factory,
            subpass,
            framebuffer_width,
            framebuffer_height,
            vec![
                env.raw_layout(),
                texture_layout.raw(),
            ],
        )?;

        // create a static vertex buffer
        let vbuf_size = FXAAVertexArgs::vertex().stride as u64 * 6;
        let mut vbuf = factory.create_buffer(
            BufferInfo {
                size: vbuf_size,
                usage: hal::buffer::Usage::VERTEX
            },
            memory::Dynamic,
        ).unwrap();
        unsafe {
            factory
                .upload_visible_buffer(
                    &mut vbuf,
                    0,
                    &[
                        FXAAVertexArgs { position:[-1f32,1f32].into(), tex_coord:[0f32,1f32].into() },
                        FXAAVertexArgs { position:[1f32,-1f32].into(), tex_coord:[1f32,0f32].into() },
                        FXAAVertexArgs { position:[-1f32,-1f32].into(), tex_coord:[0f32,0f32].into() },
                        FXAAVertexArgs { position:[1f32,-1f32].into(), tex_coord:[1f32,0f32].into() },
                        FXAAVertexArgs { position:[-1f32,1f32].into(), tex_coord:[0f32,1f32].into() },
                        FXAAVertexArgs { position:[1f32,1f32].into(), tex_coord:[1f32,1f32].into() },
                    ],
                )
                .unwrap();
        }

        Ok(Box::new(DrawFXAA::<B> {
            pipeline: pipeline,
            pipeline_layout: pipeline_layout,
            vertex_buffer: vbuf,
            env:env,
            texture_set:texture_set
        }))
    }
}

// build the pipeline
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

    // build the pipeline
    let pipes = PipelinesBuilder::new()
        .with_pipeline(
            PipelineDescBuilder::new()
                .with_vertex_desc(&[(FXAAVertexArgs::vertex(), pso::VertexInputRate::Vertex)])
                .with_shaders(util::simple_shader_set(
                    &shader_vertex,
                    Some(&shader_fragment),
                ))
                .with_layout(&pipeline_layout)
                .with_subpass(subpass)
                .with_framebuffer_size(framebuffer_width, framebuffer_height)
                .with_face_culling(pso::Face::BACK)
                // alpha blended
                .with_blend_targets(vec![pso::ColorBlendDesc {
                    mask: pso::ColorMask::ALL,
                    blend: None,
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

// uniform arguments
/// layout(std140, set = 0, binding = 0) uniform FXAAUniformArgs {
///    uniform int scale;
/// };
#[derive(Clone, Copy, Debug, AsStd140)]
#[repr(C, align(4))]
pub struct FXAAUniformArgs {
    pub screen_width: float,
    pub screen_height: float,
}

/// Vertex Arguments to pass into shader.
/// layout(location = 0) out VertexData {
///    vec2 position;
///    vec2 tex_coord;
/// } vertex;
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, AsStd140)]
#[repr(C, align(4))]
pub struct FXAAVertexArgs {
    pub position: vec2,
    pub tex_coord: vec2,
}

/// Required to send data into the shader.
/// These names must match the shader.
impl AsVertex for FXAAVertexArgs {
    fn vertex() -> VertexFormat {
        VertexFormat::new((
            (Format::Rg32Sfloat, "position"),
            (Format::Rg32Sfloat, "tex_coord"),
        ))
    }
}

// implementation of the render pass
#[derive(Debug)]
pub struct DrawFXAA<B: Backend> {
    pipeline: B::GraphicsPipeline,
    pipeline_layout: B::PipelineLayout,
    vertex_buffer: Escape<Buffer<B>>,
    env: DynamicUniform<B, FXAAUniformArgs>,
    texture_set: Escape<DescriptorSet<B>>,
}

impl<B: Backend> RenderGroup<B, World> for DrawFXAA<B> {
    fn prepare(
        &mut self,
        factory: &Factory<B>,
        _queue: QueueId,
        index: usize,
        _subpass: hal::pass::Subpass<B>,
        world: &World,
    ) -> PrepareResult {
        // write screen dimensions to the uniform
        let dimensions = world.read_resource::<ScreenDimensions>();
        self.env.write(factory, index, FXAAUniformArgs {
            screen_width: dimensions.width(),
            screen_height: dimensions.height(),
        }.std140());
        
        //PrepareResult::DrawReuse
        PrepareResult::DrawRecord
    }

    fn draw_inline(
        &mut self,
        mut encoder: RenderPassEncoder<'_, B>,
        index: usize,
        _subpass: hal::pass::Subpass<'_, B>,
        _resources: &World,
    ) {
        let layout = &self.pipeline_layout;
        let encoder = &mut encoder;

        // bind encoder
        encoder.bind_graphics_pipeline(&self.pipeline);

        // bind the dynamic uniform buffer
        self.env.bind(index, layout, 0, encoder);

        unsafe {
            // bind texture descriptor
            encoder.bind_graphics_descriptor_sets(layout, 1, Some(self.texture_set.raw()), std::iter::empty());

            // bind vertex buffer
            encoder.bind_vertex_buffers(0, Some((self.vertex_buffer.raw(), 0)));

            // and draw
            encoder.draw(0..6, 0..1);
        }
    }

    fn dispose(self: Box<Self>, factory: &mut Factory<B>, _aux: &World) {
        unsafe {
            factory.device().destroy_graphics_pipeline(self.pipeline);
            factory.device().destroy_pipeline_layout(self.pipeline_layout);
        }
    }
}