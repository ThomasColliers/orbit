mod debug;

use amethyst::{
    assets::{PrefabLoader, PrefabLoaderSystemDesc, RonFormat, PrefabData, ProgressCounter, AssetPrefab },
    core::{
        shrev::{EventChannel, ReaderId},
        Transform,TransformBundle,
        math::{Point3, Vector3},
        Time,
        frame_limiter::FrameRateLimitStrategy,
        HideHierarchySystemDesc, HiddenPropagate,
    },
    derive::{PrefabData, SystemDesc},
    ecs::{Entity, Read, ReadExpect, ReadStorage, System, SystemData, WorldExt, WriteStorage, Join},
    prelude::{
        Application, Builder, GameData, GameDataBuilder, SimpleState, SimpleTrans, StateData,
        StateEvent, Trans,
    },
    gltf::{GltfSceneLoaderSystemDesc, GltfSceneAsset, GltfSceneFormat},
    renderer::{
        camera::{Camera, CameraPrefab},
        formats::GraphicsPrefab,
        light::LightPrefab,
        debug_drawing::{ DebugLines, DebugLinesComponent, DebugLinesParams },
        plugins::{RenderPbr3D, RenderToWindow, RenderDebugLines },
        rendy::mesh::{Normal, Position, Tangent, TexCoord},
        types::DefaultBackend,
        RenderingBundle,
        palette::Srgba,
    },
    utils::{
        application_root_dir, 
        auto_fov::{AutoFov, AutoFovSystem},
        fps_counter::{FpsCounter, FpsCounterBundle},
        tag::{Tag, TagFinder},
        removal::Removal
    },
    ui::{RenderUi, UiBundle, UiCreator, UiFinder, UiText },
    input::{
        is_close_requested, is_key_down, InputBundle, StringBindings, InputEvent, ScrollDirection
    },
    controls::{ArcBallControlBundle, ArcBallControlTag, ControlTagPrefab},
    winit::VirtualKeyCode,
    Error
};
use log::{error, info};
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
}

#[derive(Default)]
struct MainState {
    fps_display: Option<Entity>,
}

impl SimpleState for MainState {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        // setup the debug lines as a resoruce
        data.world.insert(DebugLines::new());
        data.world.insert(DebugLinesParams { line_width: 0.5 });        
        // and create the component and entity
        data.world.register::<DebugLinesComponent>();

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

    fn update(&mut self, state_data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        let StateData { world, .. } = state_data;

        // get reference to the ui element
        if self.fps_display.is_none() {
            world.exec(|finder: UiFinder<'_>| {
                if let Some(entity) = finder.find("fps_text") {
                    self.fps_display = Some(entity);
                }
            })
        }

        // update the ui
        let mut ui_text = world.write_storage::<UiText>();
        {
            if let Some(fps_display) = self.fps_display.and_then(|entity| ui_text.get_mut(entity)) {
                if world.read_resource::<Time>().frame_number() % 20 == 0 {
                    let fps = world.read_resource::<FpsCounter>().sampled_fps();
                    fps_display.text = format!("FPS: {:.*}", 2, fps);
                }
            }
        }

        Trans::None
    }

    // handle events
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
            debug::DebugSystemDesc::default(),
            "debug_sytem",
            &["input_system"]
        )
        .with_bundle(
            RenderingBundle::<DefaultBackend>::new()
                .with_plugin(
                    RenderToWindow::from_config_path(display_config_path)?.with_clear([0.0, 0.0, 0.0, 1.0]),
                )
                .with_plugin(RenderPbr3D::default())
                .with_plugin(RenderDebugLines::default())
                .with_plugin(RenderUi::default()),
        )?;

    // build application and run it
    let mut game = Application::build(assets_dir, MainState::default())?
        .with_frame_limit(FrameRateLimitStrategy::Unlimited, 9999) // this doesn't seem to work at the moment
        .build(game_data)?;
    game.run();

    Ok(())
}
