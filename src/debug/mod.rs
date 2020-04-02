use amethyst::{
    core::{
        math::{Point3, Vector3},
        HiddenPropagate,
        shrev::{EventChannel, ReaderId},
    },
    renderer::{
        debug_drawing::{DebugLinesComponent},
        palette::Srgba,
    },
    derive::SystemDesc,
    ecs::prelude::{Join, Read, ReadStorage, System, SystemData, WriteStorage, Entity, Entities },
    input::{InputHandler, StringBindings, InputEvent},
    ui::{RenderUi, UiBundle, UiCreator, UiFinder, UiText},
};

#[derive(SystemDesc)]
#[system_desc(name(DebugSystemDesc))]
pub struct DebugSystem {
    #[system_desc(event_channel_reader)]
    event_reader: ReaderId<InputEvent<StringBindings>>,
}

impl DebugSystem {
    pub fn new(event_reader: ReaderId<InputEvent<StringBindings>>) -> Self {
        Self { event_reader }
    }
}

impl<'s> System<'s> for DebugSystem {
    type SystemData = (
        Read<'s, EventChannel<InputEvent<StringBindings>>>,
        UiFinder<'s>,
        WriteStorage<'s, HiddenPropagate>,
        WriteStorage<'s, DebugLinesComponent>,
        Entities<'s>,
    );

    fn run(&mut self, (events, ui_finder, mut hidden, mut debuglines, entities): Self::SystemData) {
        for event in events.read(&mut self.event_reader) {
            if let InputEvent::ActionPressed(action) = event {
                let mut toggle_entity_display = |label| {
                    if let Some(entity) = ui_finder.find(label) {
                        match hidden.get(entity) {
                            Some(_) => { hidden.remove(entity); },
                            None => { hidden.insert(entity, HiddenPropagate::new()).ok(); },
                        }
                    }
                };
                match action.as_str() {
                    "help" => { 
                        if let Some(entity) = ui_finder.find("help_container") {
                            match hidden.get(entity) {
                                Some(_) => { hidden.remove(entity); },
                                None => { hidden.insert(entity, HiddenPropagate::new()).expect("Failed to create HiddenPropagate component"); },
                            }
                        }
                    },
                    "fps" => {
                        if let Some(entity) = ui_finder.find("fps_text") {
                            match hidden.get(entity) {
                                Some(_) => { hidden.remove(entity); },
                                None => { hidden.insert(entity, HiddenPropagate::new()).expect("Failed to create HiddenPropagate component"); },
                            }
                        }
                    },
                    "debuglines" => {
                        // remove if we already have debug lines
                        let mut has_removed = false;
                        for (e, _) in (&*entities, &debuglines).join() {
                            entities.delete(e).expect("Failed to remove debug line entity");
                            has_removed = true;
                        }
                        // create if none were found
                        if !has_removed {
                            entities.build_entity().with(create_debug_lines(), &mut debuglines).build();
                        }
                    },
                    _ => ()
                }
            }
        }
    }
}

pub fn create_debug_lines() -> DebugLinesComponent {
    let mut debug_lines_component = DebugLinesComponent::with_capacity(100);
    debug_lines_component.add_direction(
        [0.0, 0.01, 0.0].into(),
        [3.0, 0.01, 0.0].into(),
        Srgba::new(1.0, 0.0, 0.0, 1.0),
    );
    debug_lines_component.add_direction(
        [0.0, 0.0, 0.0].into(),
        [0.0, 3.0, 0.0].into(),
        Srgba::new(0.0, 1.0, 0.0, 1.0),
    );
    debug_lines_component.add_direction(
        [0.0, 0.01, 0.0].into(),
        [0.0, 0.01, 3.0].into(),
        Srgba::new(0.0, 0.0, 1.0, 1.0),
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