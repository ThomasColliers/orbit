use amethyst::{
    ecs::prelude::{ Join, Component, System, SystemData, WriteStorage, Read },
    derive::{SystemDesc},
    core::{timing::Time, transform::Transform, shrev::{EventChannel, ReaderId}},
    input::{StringBindings, InputEvent, ScrollDirection},
    utils::{
        tag::{Tag},
    },
    controls::ArcBallControlTag,
};

#[derive(SystemDesc)]
#[system_desc(name(CameraControlSystemDesc))]
pub struct CameraControlSystem {
    #[system_desc(event_channel_reader)]
    event_reader: ReaderId<InputEvent<StringBindings>>,
    #[system_desc(skip)]
    target_distance:Option<f32>
}

impl CameraControlSystem {
    pub fn new(event_reader: ReaderId<InputEvent<StringBindings>>) -> Self {
        Self { event_reader:event_reader, target_distance:None }
    }
}

impl<'s> System<'s> for CameraControlSystem {
    type SystemData = (
        Read<'s, EventChannel<InputEvent<StringBindings>>>,
        WriteStorage<'s, ArcBallControlTag>,
        Read<'s, Time>,
    );

    fn run(&mut self, (events, mut arcball_control, time):Self::SystemData) {
        if let Some(current) = self.target_distance {
            // loop events
            let mut updated_value = current;
            for event in events.read(&mut self.event_reader) {
                if let InputEvent::MouseWheelMoved(direction) = event {            
                    if direction.eq(&ScrollDirection::ScrollUp) {
                        updated_value += 1.0;
                    } else if direction.eq(&ScrollDirection::ScrollDown) {
                        updated_value -= 1.0;
                    }
                }
            }
            if updated_value < 3.0 {
                updated_value = 3.0
            }
            self.target_distance = Some(updated_value);
            // make arcball value approach our value
            for item in (&mut arcball_control).join() {
                item.distance = item.distance+(updated_value-item.distance)*4.0*time.delta_seconds();
            }
        // init if not set yet
        } else {
            for item in arcball_control.join() {
                self.target_distance = Some(item.distance);
            }
        }
    }
}