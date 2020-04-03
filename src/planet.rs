use amethyst::{
    ecs::{NullStorage},
    ecs::prelude::{ Join, Component, System, SystemData, WriteStorage, ReadStorage, Read },
    derive::{SystemDesc},
    core::{timing::Time, transform::Transform},
    utils::{
        tag::{Tag},
    },
};

#[derive(Clone, Default)]
pub struct Planet;
impl Component for Planet {
    type Storage = NullStorage<Self>;
}

#[derive(Clone, Default)]
pub struct Clouds;
impl Component for Clouds {
    type Storage = NullStorage<Self>;
}

#[derive(SystemDesc)]
#[system_desc(name(PlanetSystemDesc))]
pub struct PlanetSystem;

impl<'s> System<'s> for PlanetSystem {
    type SystemData = (
        ReadStorage<'s, Tag<Planet>>,
        ReadStorage<'s, Tag<Clouds>>,
        WriteStorage<'s, Transform>,
        Read<'s, Time>,
    );

    fn run(&mut self, (_planets, clouds, mut transforms, time) : Self::SystemData) {
        for (_, transform) in (&clouds, &mut transforms).join() {
            transform.append_rotation_y_axis(0.015 * time.delta_seconds());
        }
    }
}