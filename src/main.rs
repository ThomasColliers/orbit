use amethyst::{
    assets::{PrefabLoader, PrefabLoaderSystemDesc, RonFormat, PrefabData, ProgressCounter },
    core::{
        shrev::{EventChannel, ReaderId},
        Transform,TransformBundle
    },
    derive::{PrefabData, SystemDesc},
    ecs::{Entity, Read, ReadExpect, ReadStorage, System, SystemData, WorldExt, WriteStorage, Join},
    prelude::{
        Application, Builder, GameData, GameDataBuilder, SimpleState, SimpleTrans, StateData,
        StateEvent, Trans,
    },
    renderer::{
        camera::{Camera, CameraPrefab},
        formats::GraphicsPrefab,
        light::LightPrefab,
        plugins::{RenderShaded3D, RenderToWindow},
        rendy::mesh::{Normal, Position, Tangent, TexCoord},
        types::DefaultBackend,
        RenderingBundle,
    },
    utils::{
        application_root_dir, 
        auto_fov::{AutoFov, AutoFovSystem},
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
    transform: Option<Transform>,
    light: Option<LightPrefab>,
    camera: Option<CameraPrefab>,
    auto_fov: Option<AutoFov>,
    control_tag: Option<ControlTagPrefab>,
    show_fov_tag: Option<Tag<ShowFov>>,
}

#[derive(Clone, Default)]
struct ShowFov;

struct MainState;

impl SimpleState for MainState {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        // load the scene from the ron file
        let handle = data.world.exec(|loader: PrefabLoader<'_, ScenePrefab>| {
            loader.load("scene.ron", RonFormat, ())
        });
        data.world.create_entity().with(handle).build();
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

#[derive(SystemDesc)]
#[system_desc(name(CameraDistanceSystemDesc))]
struct CameraDistanceSystem {
    #[system_desc(event_channel_reader)]
    event_reader: ReaderId<InputEvent<StringBindings>>,
}
impl CameraDistanceSystem {
    pub fn new(event_reader: ReaderId<InputEvent<StringBindings>>) -> Self {
        CameraDistanceSystem { event_reader }
    }
}
impl<'a> System<'a> for CameraDistanceSystem {
    type SystemData = (
        Read<'a, EventChannel<InputEvent<StringBindings>>>,
        ReadStorage<'a, Transform>,
        WriteStorage<'a, ArcBallControlTag>,
    );

    fn run(&mut self, (events, transforms, mut tags): Self::SystemData) {
        for event in events.read(&mut self.event_reader) {
            if let InputEvent::MouseWheelMoved(direction) = *event {
                match direction {
                    ScrollDirection::ScrollUp => {
                        for (_, tag) in (&transforms, &mut tags).join() {
                            tag.distance *= 0.9;
                            println!("scroll up {}", tag.distance);
                        }
                    }
                    ScrollDirection::ScrollDown => {
                        for (_, tag) in (&transforms, &mut tags).join() {
                            tag.distance *= 1.1;
                            println!("scroll down {}", tag.distance);
                        }
                    }
                    _ => (),
                }
            }
        }
    }
}

fn main() -> amethyst::Result<()> {
    // start logging
    amethyst::start_logger(Default::default());

    // directories
    let app_root = application_root_dir()?;
    let assets_dir = app_root.join("assets");
    let config_dir = app_root.join("config");
    let display_config_path = config_dir.join("display.ron");
    let key_bindings_path = config_dir.join("input.ron");

    // build gamedata
    let game_data = GameDataBuilder::default()
        .with_system_desc(
            PrefabLoaderSystemDesc::<ScenePrefab>::default(), 
            "prefab", 
            &[]
        )
        .with(AutoFovSystem::new(), "auto_fov", &["prefab"])
        .with_bundle(TransformBundle::new())?
        .with_bundle(
            InputBundle::<StringBindings>::new().with_bindings_from_file(&key_bindings_path)?,
        )?
        .with_bundle(ArcBallControlBundle::<StringBindings>::new())?
        .with_system_desc(
            CameraDistanceSystemDesc::default(),
            "camera_distance_system",
            &["input_system"],
        )
        .with_bundle(UiBundle::<StringBindings>::new())?
        .with_bundle(
            RenderingBundle::<DefaultBackend>::new()
                .with_plugin(
                    RenderToWindow::from_config_path(display_config_path)?.with_clear([0.0, 0.0, 0.0, 1.0]),
                )
                .with_plugin(RenderShaded3D::default())
                .with_plugin(RenderUi::default()),
        )?;

    let mut game = Application::new(assets_dir, MainState, game_data)?;
    game.run();
    Ok(())
}
