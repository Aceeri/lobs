use avian3d::prelude::*;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy_trenchbroom::brush::ConvexHull;
use bevy_trenchbroom::geometry::{Brushes, BrushesAsset};
use bevy_trenchbroom::prelude::*;

use super::npc::{Body, NpcRegistry};
use crate::screens::Screen;
use crate::third_party::avian3d::CollisionLayer;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (init_graves, slot_bodies_in_graves));
    app.add_systems(
        Update,
        tutorial_spawn.run_if(in_state(Screen::Gameplay)),
    );
    app.add_observer(init_body_spawner);
    app.add_observer(on_spawn_body);
}

#[derive(Resource)]
struct TutorialSpawnTimer(Timer);

impl Default for TutorialSpawnTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(10.0, TimerMode::Once))
    }
}

fn tutorial_spawn(
    time: Res<Time>,
    mut timer: Local<TutorialSpawnTimer>,
    mut commands: Commands,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        info!("Tutorial spawn triggered on 'tutorial_spawner'");
        commands.trigger(SpawnBody::Direct {
            spawner_name: "tutorial_spawner".into(),
            npc_name: "lobster".into(),
        });
    }
}

#[solid_class(base(Transform, Visibility))]
pub(crate) struct Grave {
    pub slots: u32,
}

impl Default for Grave {
    fn default() -> Self {
        Self { slots: 1 }
    }
}

#[derive(Component)]
struct GraveState {
    slots: u32,
    filled: u32,
}

#[derive(Component)]
struct GraveSensor(Entity);

#[derive(Component)]
pub(crate) struct GraveSlotted;

fn init_graves(
    mut commands: Commands,
    graves: Query<(Entity, &Grave, &Brushes), Without<GraveState>>,
    brushes_assets: Res<Assets<BrushesAsset>>,
) {
    for (entity, grave, brushes) in &graves {
        let brushes_asset = match brushes {
            Brushes::Owned(asset) => asset,
            Brushes::Shared(handle) => {
                let Some(asset) = brushes_assets.get(handle) else {
                    continue;
                };
                asset
            }
            #[allow(unreachable_patterns)]
            _ => continue,
        };

        let mut min = DVec3::INFINITY;
        let mut max = DVec3::NEG_INFINITY;
        for brush in brushes_asset.iter() {
            if let Some((from, to)) = brush.as_cuboid() {
                min = min.min(from);
                max = max.max(to);
            } else {
                for (vertex, _) in brush.calculate_vertices() {
                    min = min.min(vertex);
                    max = max.max(vertex);
                }
            }
        }

        if !min.is_finite() || !max.is_finite() {
            continue;
        }

        let size = (max - min).as_vec3();
        let center = ((min + max) * 0.5).as_vec3();

        commands.entity(entity).insert(GraveState {
            slots: grave.slots,
            filled: 0,
        });

        commands.spawn((
            GraveSensor(entity),
            Collider::cuboid(size.x, size.y, size.z),
            Sensor,
            CollisionLayers::new(CollisionLayer::Sensor, CollisionLayer::Character),
            Transform::from_translation(center),
            CollidingEntities::default(),
        ));
    }
}

#[point_class(base(Transform, Visibility))]
pub(crate) struct BodySpawner {
    pub name: String,
    /// CSV list, e.g. "lobster,lobster,pistol shrimp" -> ["lobster", "lobster", "pistol shrimp"]
    /// can leave empty in trenchbroom if we just want a spawn location too
    pub queue: String,
}

impl Default for BodySpawner {
    fn default() -> Self {
        Self {
            name: String::new(),
            queue: String::new(),
        }
    }
}

#[derive(Component)]
struct SpawnerState {
    queue: Vec<String>,
    index: usize,
}

fn init_body_spawner(
    add: On<Add, BodySpawner>,
    mut commands: Commands,
    spawners: Query<&BodySpawner>,
) {
    let Ok(spawner) = spawners.get(add.entity) else {
        return;
    };
    let queue: Vec<String> = spawner
        .queue
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    commands
        .entity(add.entity)
        .insert(SpawnerState { queue, index: 0 });
}

#[derive(Event)]
pub(crate) enum SpawnBody {
    Queue {
        spawner_name: String,
    },
    Direct {
        spawner_name: String,
        npc_name: String,
    },
}

const BODY_SPAWN_SPEED: f32 = 5.0;

fn on_spawn_body(
    event: On<SpawnBody>,
    mut commands: Commands,
    mut spawners: Query<(&BodySpawner, &GlobalTransform, &mut SpawnerState)>,
    registry: Res<NpcRegistry>,
    assets: Res<AssetServer>,
) {
    let (target_spawner, target_npc): (&str, Option<&str>) = match &*event {
        SpawnBody::Queue { spawner_name } => (spawner_name.as_str(), None),
        SpawnBody::Direct {
            spawner_name,
            npc_name,
        } => (spawner_name.as_str(), Some(npc_name.as_str())),
    };

    for (spawner, transform, mut state) in &mut spawners {
        if spawner.name != target_spawner {
            continue;
        }

        let npc_name = match target_npc {
            Some(name) => name.to_string(),
            None => {
                if state.queue.is_empty() {
                    continue;
                }
                let name = state.queue[state.index].clone();
                state.index = (state.index + 1) % state.queue.len();
                name
            }
        };

        let Some(prefab) = registry.prefabs.get(&npc_name) else {
            warn!("NPC '{}' not found in registry", npc_name);
            continue;
        };

        let t = transform.compute_transform();
        let forward = t.forward();

        commands
            .spawn((
                Body,
                Collider::cylinder(prefab.radius, prefab.height),
                ColliderDensity(1_000.0),
                RigidBody::Dynamic,
                CollisionLayers::new(
                    [CollisionLayer::Character, CollisionLayer::Prop],
                    [CollisionLayer::Default, CollisionLayer::Prop],
                ),
                LinearVelocity(*forward * BODY_SPAWN_SPEED),
                t,
            ))
            .with_child((
                Name::new("Body Model"),
                SceneRoot(assets.load(prefab.scene.clone())),
                Transform::from_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
            ));
    }
}

fn slot_bodies_in_graves(
    mut commands: Commands,
    sensors: Query<(&GraveSensor, &CollidingEntities)>,
    mut graves: Query<&mut GraveState>,
    bodies: Query<Entity, (With<Body>, Without<GraveSlotted>)>,
) {
    for (sensor, colliding) in &sensors {
        let Ok(mut state) = graves.get_mut(sensor.0) else {
            continue;
        };

        for &colliding_entity in colliding.iter() {
            if state.filled >= state.slots {
                break;
            }

            if bodies.get(colliding_entity).is_ok() {
                state.filled += 1;
                commands
                    .entity(colliding_entity)
                    .remove::<(RigidBody, Collider, CollisionLayers)>()
                    .insert(GraveSlotted);
            }
        }
    }
}
