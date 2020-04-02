use amethyst::{
    core::{
        math::{Point3, Vector3},
        HiddenPropagate,
    },
    renderer::{
        debug_drawing::{DebugLinesComponent},
        palette::Srgba,
    },
    derive::SystemDesc,
    ecs::prelude::{Join, Read, ReadStorage, System, SystemData, WriteStorage},
    input::{InputHandler, StringBindings},
    ui::{RenderUi, UiBundle, UiCreator, UiFinder, UiText},
};

#[derive(SystemDesc)]
#[system_desc(name(DebugSystemDesc))]
pub struct DebugSystem;

impl DebugSystem {
    pub fn new() -> Self {
        Self {}
    }
}

impl<'s> System<'s> for DebugSystem {
    type SystemData = (
        Read<'s, InputHandler<StringBindings>>,
        UiFinder<'s>,
        WriteStorage<'s, HiddenPropagate>,
    );

    fn run(&mut self, (input, ui_finder, mut hidden): Self::SystemData) {
        // help button
        let help_pressed = input.action_is_down("help").unwrap_or(false);
        if help_pressed {
            println!("help pressed");
            if let Some(entity) = ui_finder.find("help_container") {
                match hidden.get(entity) {
                    Some(_) => { println!("show"); hidden.remove(entity); },
                    None => { println!("hide"); hidden.insert(entity, HiddenPropagate::new()).ok(); },
                }
            }
        }
    }
}

pub fn create_debug_lines() -> DebugLinesComponent {
    let mut debug_lines_component = DebugLinesComponent::with_capacity(100);
    debug_lines_component.add_direction(
        [0.0, 0.0001, 0.0].into(),
        [3.0, 0.0, 0.0].into(),
        Srgba::new(1.0, 0.0, 0.0, 1.0),
    );
    debug_lines_component.add_direction(
        [0.0, 0.0, 0.0].into(),
        [0.0, 3.0, 0.0].into(),
        Srgba::new(0.0, 1.0, 0.0, 1.0),
    );
    debug_lines_component.add_direction(
        [0.0, 0.0001, 0.0].into(),
        [0.0, 0.0, 3.0].into(),
        Srgba::new(0.0, 0.0, 0.1, 1.0),
    );
    let width: u32 = 10;
    let depth: u32 = 10;
    let main_color = Srgba::new(0.4, 0.4, 0.4, 1.0);
    // Grid lines in X-axis
    for x in 0..=width {
        let (x, width, depth) = (x as f32, width as f32, depth as f32);
        let position = Point3::new(x - width / 2.0, 0.0, -depth / 2.0);
        let direction = Vector3::new(0.0, 0.0, depth);
        debug_lines_component.add_direction(position, direction, main_color);
    }
    // Grid lines in Z-axis
    for z in 0..=depth {
        let (z, width, depth) = (z as f32, width as f32, depth as f32);
        let position = Point3::new(-width / 2.0, 0.0, z - depth / 2.0);
        let direction = Vector3::new(width, 0.0, 0.0);
        debug_lines_component.add_direction(position, direction, main_color);
    }
    debug_lines_component
}