mod debug;
mod planet;
mod render;
mod controls;

use amethyst::{
    assets::{PrefabLoader, PrefabLoaderSystemDesc, RonFormat, PrefabData, ProgressCounter, AssetPrefab, Processor },
    core::{
        Transform,TransformBundle,
        frame_limiter::FrameRateLimitStrategy,
        HideHierarchySystemDesc,
    },
    derive::{PrefabData},
    ecs::{Entity, WorldExt},
    prelude::{
        Application, Builder, GameData, GameDataBuilder, SimpleState, SimpleTrans, StateData,
        StateEvent, Trans,
    },
    gltf::{GltfSceneLoaderSystemDesc, GltfSceneAsset, GltfSceneFormat},
    renderer::{
        camera::{CameraPrefab},
        formats::GraphicsPrefab,
        light::LightPrefab,
        debug_drawing::{ DebugLines, DebugLinesComponent, DebugLinesParams },
        plugins::{RenderPbr3D, RenderToWindow, RenderDebugLines },
        rendy::mesh::{Normal, Position, Tangent, TexCoord},
        types::DefaultBackend,
        mtl::Material,
        RenderingBundle,
        bundle::Target,
        MeshProcessorSystem, TextureProcessorSystem, visibility::VisibilitySortingSystem,
        RenderingSystem
    },
    utils::{
        application_root_dir, 
        auto_fov::{AutoFov, AutoFovSystem},
        fps_counter::{FpsCounterBundle},
        tag::{Tag},
    },
    ui::{ UiBundle, UiCreator, UiGlyphsSystemDesc },
    input::{
        is_close_requested, is_key_down, InputBundle, StringBindings
    },
    window::{WindowBundle},
    controls::{ArcBallControlBundle, ControlTagPrefab},
    winit::VirtualKeyCode,
    Error
};
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, PrefabData, Serialize)]
#[serde(default)]
struct ScenePrefab {
    graphics: Option<GraphicsPrefab<(Vec<Position>, Vec<Normal>, Vec<Tangent>, Vec<TexCoord>)>>,
    gltf: Option<AssetPrefab<GltfSceneAsset, GltfSceneFormat>>,
    transform: Option<Transform>,
    light: Option<LightPrefab>,
    camera: Option<CameraPrefab>,
    auto_fov: Option<AutoFov>,
    control_tag: Option<ControlTagPrefab>,
    planet: Option<Tag<planet::Planet>>,
    clouds: Option<Tag<planet::Clouds>>,
    atmosphere: Option<Tag<render::atmosphere::Atmosphere>>,
    sun: Option<Tag<render::sun::Sun>>,
}

#[derive(Default)]
struct MainState;

impl SimpleState for MainState {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        // setup the debug lines as a resoruce
        data.world.insert(DebugLines::new());
        data.world.insert(DebugLinesParams { line_width: 0.5 });        
        // and create the component and entity
        data.world.register::<DebugLinesComponent>();
        data.world.register::<debug::FpsDisplay>();

        // set fxaa enabled
        data.world.insert(render::fxaa::FxaaSettings { enabled:true });
        data.world.insert(render::tonemap::TonemapSettings { enabled:true, exposure:1.0 });

        // register custom components
        data.world.register::<planet::Planet>();
        data.world.register::<planet::Clouds>();
        data.world.register::<render::atmosphere::Atmosphere>();
        data.world.register::<render::sun::Sun>();

        // load the scene from the ron file
        let handle = data.world.exec(|loader: PrefabLoader<'_, ScenePrefab>| {
            loader.load("scene.ron", RonFormat, ())
        });
        data.world.create_entity().with(handle).build();

        // load the ui
        data.world.exec(|mut creator: UiCreator<'_>| {
            creator.create("ui.ron",());
        });
    }

    fn update(&mut self, _data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        //let StateData { world, .. } = state_data;
        Trans::None
    }

    // handle application level events
    fn handle_event(&mut self, _data: StateData<'_, GameData<'_, '_>>, event: StateEvent) -> SimpleTrans {
        if let StateEvent::Window(ref event) = event {
            if is_close_requested(event) || is_key_down(event, VirtualKeyCode::Escape) {
                Trans::Quit
            } else {
                Trans::None
            }
        } else {
            Trans::None
        }
    }
}

fn main() -> amethyst::Result<()> {
    // start logging
    amethyst::start_logger(Default::default());

    // directories and configuration files
    let app_root = application_root_dir()?;
    let assets_dir = app_root.join("assets");
    let config_dir = app_root.join("config");
    let display_config_path = config_dir.join("display.ron");
    let key_bindings_path = {
        if cfg!(feature = "sdl_controller") {
            assets_dir.join("input_controller.ron")
        } else {
            assets_dir.join("input.ron")
        }
    };

    // build gamedata
    let game_data = GameDataBuilder::default()
        .with_system_desc(
            PrefabLoaderSystemDesc::<ScenePrefab>::default(), 
            "scene_loader", 
            &[]
        )
        .with_system_desc(
            GltfSceneLoaderSystemDesc::default(),
            "gltf_loader",
            &["scene_loader"]
        )
        .with_bundle(TransformBundle::new())?
        .with_system_desc(
            HideHierarchySystemDesc::default(),
            "hide_hierarchy_system",
            &["parent_hierarchy_system"]
        )
        .with(AutoFovSystem::new(), "auto_fov", &["scene_loader"])
        .with_bundle(
            InputBundle::<StringBindings>::new().with_bindings_from_file(key_bindings_path)?,
        )?
        .with_bundle(UiBundle::<StringBindings>::new())?
        .with_bundle(FpsCounterBundle::default())?
        .with_bundle(ArcBallControlBundle::<StringBindings>::new())?
        .with_system_desc(
            controls::CameraControlSystemDesc::default(),
            "camera_controls",
            &["input_system"],
        )
        .with_system_desc(
            debug::DebugSystemDesc::default(),
            "debug_sytem",
            &["input_system"]
        )
        .with_system_desc(
            planet::PlanetSystemDesc::default(),
            "planet_system",
            &[]
        )
        .with_system_desc(
            UiGlyphsSystemDesc::<DefaultBackend>::default(),
            "ui_glyph_system",
            &[],
        )
        .with(
            VisibilitySortingSystem::new(),
            "visibility_sorting_system",
            &[],
        )
        .with(
            MeshProcessorSystem::<DefaultBackend>::default(),
            "mesh_processor",
            &[],
        )
        .with(
            TextureProcessorSystem::<DefaultBackend>::default(),
            "texture_processor",
            &[],
        )
        .with(Processor::<Material>::new(), "material_processor", &[])
        .with_bundle(WindowBundle::from_config_path(display_config_path)?)?
        .with_thread_local(RenderingSystem::<DefaultBackend, _>::new(
            render::graph::RenderGraph::default(),
        ));

    // build application and run it
    let mut game = Application::build(assets_dir, MainState::default())?
        //.with_frame_limit(FrameRateLimitStrategy::Unlimited, 9999) // this eats all available CPU cycles
        .with_frame_limit(
            FrameRateLimitStrategy::SleepAndYield(Duration::from_millis(2)),
            144,
        )
        .build(game_data)?;
    game.run();

    Ok(())
}
