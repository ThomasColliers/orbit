// tonemapping render pipeline
use amethyst::{
    ecs::{World},
    prelude::*,
    window::ScreenDimensions,
};
use rendy::{
    command::{QueueId, RenderPassEncoder },
    hal::{
        self, 
        device::Device, pso::ShaderStageFlags, pso::DescriptorPool,
        format::Format, image::Filter::Linear, image::WrapMode 
    },
    graph::{
        render::{
            PrepareResult,
            SimpleGraphicsPipelineDesc,
            SimpleGraphicsPipeline,
            Layout, SetLayout
        },
        GraphContext, NodeBuffer, NodeImage, ImageAccess,
    },
    mesh::{
        VertexFormat, AsVertex
    },
    shader::{SpirvShader},
    memory,
    resource::{ 
        self,Escape,BufferInfo,Buffer,
        Handle as RendyHandle,DescriptorSetLayout,
        ImageViewInfo,SamplerInfo,ImageView,Sampler,
    },
    factory::{Factory},
};
use glsl_layout::*;
use std::mem::size_of;

// tonemapping settings resource
#[derive(Default)]
pub struct TonemapSettings {
    pub enabled: bool,
    pub exposure: f32,
}

// shader pair
lazy_static::lazy_static! {
    static ref VERTEX:SpirvShader = SpirvShader::from_bytes(
        include_bytes!("../../assets/shader/fsquad.vert.spv"),
        ShaderStageFlags::VERTEX,
        "main",
    ).unwrap();

    static ref FRAGMENT:SpirvShader = SpirvShader::from_bytes(
        include_bytes!("../../assets/shader/tonemap.frag.spv"),
        ShaderStageFlags::FRAGMENT,
        "main",
    ).unwrap();

    static ref SHADERS: rendy::shader::ShaderSetBuilder = rendy::shader::ShaderSetBuilder::default()
        .with_vertex(&*VERTEX).unwrap()
        .with_fragment(&*FRAGMENT).unwrap();
}

// uniform args
#[derive(Clone, Copy, Debug, AsStd140)]
#[repr(C, align(4))]
pub struct TonemapUniformArgs {
    pub enabled: boolean,
    pub exposure: float,
}

// vertex args
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, AsStd140)]
#[repr(C, align(4))]
pub struct TonemapVertexArgs {
    pub position: vec2,
    pub tex_coord: vec2,
}

/// Required to send data into the shader.
/// These names must match the shader.
impl AsVertex for TonemapVertexArgs {
    fn vertex() -> VertexFormat {
        VertexFormat::new((
            (Format::Rg32Sfloat, "position"),
            (Format::Rg32Sfloat, "tex_coord"),
        ))
    }
}

// the pipeline itself
#[derive(Debug, Default)]
pub struct PipelineDesc;

#[derive(Debug)]
pub struct Pipeline<B: hal::Backend> {
    buffer: Escape<Buffer<B>>,
    sets: Vec<B::DescriptorSet>,
    descriptor_pool: B::DescriptorPool,
    image_sampler: Escape<Sampler<B>>,
    image_view: Escape<ImageView<B>>,
    vertex_buffer: Escape<Buffer<B>>,
    settings: Settings,
}

// utility to calculte the uniform size and offset including alignment
#[derive(Debug, PartialEq, Eq)]
struct Settings {
    align: u64
}

impl Settings {
    const UNIFORM_SIZE:u64 = size_of::<TonemapUniformArgs>() as u64;

    #[inline]
    fn buffer_frame_size(&self) -> u64 {
        ((Self::UNIFORM_SIZE - 1) / self.align + 1) * self.align
    }

    #[inline]
    fn uniform_offset(&self, index: u64) -> u64 {
        self.buffer_frame_size() * index as u64
    }
}

impl<B> SimpleGraphicsPipelineDesc<B, World> for PipelineDesc
where B: hal::Backend {
    type Pipeline = Pipeline<B>;

    fn images(&self) -> Vec<ImageAccess> {
        vec![ImageAccess {
            access: hal::image::Access::SHADER_READ,
            usage: hal::image::Usage::SAMPLED,
            layout: hal::image::Layout::ShaderReadOnlyOptimal,
            stages: hal::pso::PipelineStage::FRAGMENT_SHADER,
        }]
    }

    fn depth_stencil(&self) -> Option<hal::pso::DepthStencilDesc> {
        None
    }

    fn vertices(
        &self,
    ) -> Vec<(
        Vec<hal::pso::Element<hal::format::Format>>,
        hal::pso::ElemStride,
        hal::pso::VertexInputRate,
    )> {
        vec![
            TonemapVertexArgs::vertex().gfx_vertex_input_desc(hal::pso::VertexInputRate::Vertex),
        ]
    }

    fn load_shader_set(
        &self,
        factory: &mut Factory<B>,
        _world: &World,
    ) -> rendy::shader::ShaderSet<B> {
        SHADERS.build(factory, Default::default()).unwrap()
    }

    fn layout(&self) -> Layout {
        Layout {
            sets: vec![SetLayout {
                bindings: vec![
                    hal::pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: hal::pso::DescriptorType::UniformBuffer,
                        count: 1,
                        stage_flags: hal::pso::ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                    hal::pso::DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: hal::pso::DescriptorType::CombinedImageSampler,
                        count: 1,
                        stage_flags: hal::pso::ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                ],
            }],
            push_constants: Vec::new(),
        }
    }

    fn build<'a>(
        self,
        ctx: &GraphContext<B>,
        factory: &mut Factory<B>,
        _queue: QueueId,
        _world: &World,
        buffers: Vec<NodeBuffer>,
        images: Vec<NodeImage>,
        set_layouts: &[RendyHandle<DescriptorSetLayout<B>>],
    ) -> Result<Pipeline<B>, failure::Error> {
        assert!(buffers.is_empty());
        assert!(images.len() == 1);
        assert!(set_layouts.len() == 1);

        let align_limit = hal::adapter::PhysicalDevice::limits(factory.physical()).min_uniform_buffer_offset_alignment;
        let settings = Settings { align:align_limit };
        let frames = 3;

        let mut descriptor_pool = unsafe {
            factory.create_descriptor_pool(
                frames,
                vec![
                    hal::pso::DescriptorRangeDesc {
                        ty: hal::pso::DescriptorType::UniformBuffer,
                        count: frames,
                    },
                    hal::pso::DescriptorRangeDesc {
                        ty: hal::pso::DescriptorType::CombinedImageSampler,
                        count: frames,
                    },
                ],
                hal::pso::DescriptorPoolCreateFlags::empty(),
            )?
        };

        let image_sampler = factory
            .create_sampler(SamplerInfo {
                min_filter:Linear,
                mag_filter:Linear,
                mip_filter:Linear,
                wrap_mode:(WrapMode::Clamp,WrapMode::Clamp,WrapMode::Clamp),
                lod_bias:hal::image::Lod::ZERO,
                lod_range:hal::image::Lod::ZERO .. hal::image::Lod::MAX,
                comparison:None,
                border:[0.0,0.0,0.0,0.0].into(),
                normalized:true,
                anisotropic:hal::image::Anisotropic::Off
            })
            .unwrap();

        let image_handle = ctx
            .get_image(images[0].id)
            .expect("Input image missing");

        let image_view = factory
            .create_image_view(
                image_handle.clone(),
                ImageViewInfo {
                    view_kind: resource::ViewKind::D2,
                    format: hal::format::Format::Rgba32Sfloat, //Rgba8Unorm,
                    swizzle: hal::format::Swizzle::NO,
                    range: images[0].range.clone(),
                },
            )
            .expect("Could not create input image view");

        let buffer = factory
            .create_buffer(
                BufferInfo {
                    size: settings.buffer_frame_size() * frames as u64,
                    usage: hal::buffer::Usage::UNIFORM,
                },
                rendy::memory::MemoryUsageValue::Dynamic,
            )
            .unwrap();

        let mut sets = Vec::with_capacity(frames);
        for index in 0..frames {
            unsafe {
                let set = descriptor_pool.allocate_set(&set_layouts[0].raw()).unwrap();
                factory.write_descriptor_sets(vec![
                    hal::pso::DescriptorSetWrite {
                        set: &set,
                        binding: 0,
                        array_offset: 0,
                        descriptors: Some(hal::pso::Descriptor::Buffer(
                            buffer.raw(),
                            Some(settings.uniform_offset(index as u64))
                            ..Some(
                                settings.uniform_offset(index as u64) + Settings::UNIFORM_SIZE,
                            ),
                        )),
                    },
                    hal::pso::DescriptorSetWrite {
                        set: &set,
                        binding: 1,
                        array_offset: 0,
                        descriptors: Some(hal::pso::Descriptor::CombinedImageSampler(
                            image_view.raw(),
                            hal::image::Layout::ShaderReadOnlyOptimal,
                            image_sampler.raw()
                        )),
                    }
                ]);
                sets.push(set);
            }
        }

        // create a static vertex buffer
        let vbuf_size = TonemapVertexArgs::vertex().stride as u64 * 6;
        let mut vertex_buffer = factory.create_buffer(
            BufferInfo {
                size: vbuf_size,
                usage: hal::buffer::Usage::VERTEX
            },
            memory::Dynamic,
        ).unwrap();
        unsafe {
            factory
                .upload_visible_buffer(
                    &mut vertex_buffer,
                    0,
                    &[
                        TonemapVertexArgs { position:[-1f32,1f32].into(), tex_coord:[0f32,1f32].into() },
                        TonemapVertexArgs { position:[1f32,-1f32].into(), tex_coord:[1f32,0f32].into() },
                        TonemapVertexArgs { position:[-1f32,-1f32].into(), tex_coord:[0f32,0f32].into() },
                        TonemapVertexArgs { position:[1f32,-1f32].into(), tex_coord:[1f32,0f32].into() },
                        TonemapVertexArgs { position:[-1f32,1f32].into(), tex_coord:[0f32,1f32].into() },
                        TonemapVertexArgs { position:[1f32,1f32].into(), tex_coord:[1f32,1f32].into() },
                    ],
                )
                .unwrap();
        }

        Ok(Pipeline {
            buffer,
            sets,
            image_view,
            image_sampler,
            descriptor_pool,
            settings,
            vertex_buffer,
        })
    }
}

impl<B> SimpleGraphicsPipeline<B, World> for Pipeline<B>
where
    B: hal::Backend,
{
    type Desc = PipelineDesc;

    fn prepare(
        &mut self,
        factory: &Factory<B>,
        _queue: QueueId,
        _set_layouts: &[RendyHandle<DescriptorSetLayout<B>>],
        index: usize,
        world: &World,
    ) -> PrepareResult {
        let dimensions = world.read_resource::<ScreenDimensions>();
        let tonemap_settings = world.read_resource::<TonemapSettings>();

        // write to the uniform
        unsafe {
            factory
                .upload_visible_buffer(
                    &mut self.buffer,
                    self.settings.uniform_offset(index as u64),
                    &[TonemapUniformArgs {
                        enabled: tonemap_settings.enabled.into(),
                        exposure: tonemap_settings.exposure.into(),
                    }.std140()],
                )
                .unwrap()
        };
        
        PrepareResult::DrawRecord
    }

    fn draw(
        &mut self,
        layout: &B::PipelineLayout,
        mut encoder: RenderPassEncoder<'_, B>,
        index: usize,
        _world: &World,
    ) {
        unsafe {
            encoder.bind_graphics_descriptor_sets(
                layout,
                0,
                Some(&self.sets[index]),
                std::iter::empty(),
            );

            encoder.bind_vertex_buffers(0, Some((self.vertex_buffer.raw(), 0)));

            encoder.draw(0..6, 0..1);
        }
    }

    fn dispose(mut self, factory: &mut Factory<B>, _world: &World) {
        unsafe {
            self.descriptor_pool.reset();
            factory.destroy_descriptor_pool(self.descriptor_pool);
        }
    }
}