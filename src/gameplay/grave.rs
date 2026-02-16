use avian3d::prelude::*;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy_trenchbroom::brush::ConvexHull;
use bevy_trenchbroom::geometry::{Brushes, BrushesAsset};
use bevy_trenchbroom::prelude::*;

use super::dig::{VoxelGraves, VoxelWorldBounds};
use super::npc::{Body, NpcRegistry};
use super::tags::Tags;
use crate::gameplay::crusts::Crusts;
use crate::third_party::avian3d::CollisionLayer;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            init_graves,
            link_graves_to_voxels,
            make_grave_colliders_sensors,
            slot_bodies_in_graves,
            lerp_slotted_bodies,
            grave_reward,
            respawn_fallen_bodies,
        ),
    );
    app.add_observer(init_body_spawner);
    app.add_observer(on_spawn_body);
}

#[solid_class(base(Transform, Visibility))]
pub(crate) struct Grave {
    pub slots: u32,
    pub tags: String,
}

impl Default for Grave {
    fn default() -> Self {
        Self {
            slots: 1,
            tags: String::new(),
        }
    }
}

#[derive(Component)]
pub(crate) struct GraveState {
    pub(crate) slots: u32,
    pub(crate) filled: u32,
    pub(crate) rewarded: u32,
}

impl GraveState {
    pub fn filled(&self) -> bool {
        self.filled >= self.slots
    }
}

#[derive(Component)]
pub(crate) struct GraveVoxelVolume(pub Entity);

#[derive(Component)]
struct GraveCenter(Vec3);

#[derive(Component)]
struct GraveSensor(Entity);

#[derive(Component)]
pub(crate) struct Slotted;

#[derive(Component)]
struct GraveLerp {
    target_y: f32,
}

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

        commands.entity(entity).insert((
            GraveState {
                slots: grave.slots,
                filled: 0,
                rewarded: 0,
            },
            Tags::from_csv(&grave.tags),
            GraveCenter(center),
        ));

        commands.spawn((
            GraveSensor(entity),
            Collider::cuboid(size.x, size.y, size.z),
            Sensor,
            CollisionLayers::new(
                CollisionLayer::Sensor,
                [
                    CollisionLayer::Character,
                    CollisionLayer::Prop,
                    CollisionLayer::Ragdoll,
                ],
            ),
            Transform::from_translation(center),
            CollidingEntities::default(),
        ));
    }
}

fn link_graves_to_voxels(
    mut commands: Commands,
    unlinked_graves: Query<(Entity, &GraveCenter), (With<GraveState>, Without<GraveVoxelVolume>)>,
    mut voxel_volumes: Query<(Entity, &VoxelWorldBounds, &mut VoxelGraves)>,
) {
    for (grave_entity, grave_center) in &unlinked_graves {
        for (voxel_entity, bounds, mut graves) in &mut voxel_volumes {
            if grave_center.0.x >= bounds.min.x
                && grave_center.0.x <= bounds.max.x
                && grave_center.0.y >= bounds.min.y
                && grave_center.0.y <= bounds.max.y
                && grave_center.0.z >= bounds.min.z
                && grave_center.0.z <= bounds.max.z
            {
                commands
                    .entity(grave_entity)
                    .insert(GraveVoxelVolume(voxel_entity));
                graves.0.push(grave_entity);
                break;
            }
        }
    }
}

fn make_grave_colliders_sensors(
    mut commands: Commands,
    graves: Query<Entity, With<GraveState>>,
    q_children: Query<&Children>,
    q_needs_fix: Query<(), (With<Collider>, Without<Sensor>)>,
) {
    for grave in &graves {
        for entity in std::iter::once(grave).chain(q_children.iter_descendants(grave)) {
            if q_needs_fix.contains(entity) {
                commands
                    .entity(entity)
                    .insert((
                        Sensor,
                        CollisionLayers::new(
                            CollisionLayer::Sensor,
                            [
                                CollisionLayer::Character,
                                CollisionLayer::Prop,
                                CollisionLayer::Ragdoll,
                            ],
                        ),
                    ))
                    .remove::<RigidBody>();
            }
        }
    }
}

#[point_class(base(Transform, Visibility))]
pub(crate) struct BodySpawner {
    pub name: String,
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
    spawned: Vec<(Entity, String)>,
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
    commands.entity(add.entity).insert(SpawnerState {
        queue,
        index: 0,
        spawned: Vec::new(),
    });
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

fn body_display_name(model_key: &str) -> String {
    let mut c = model_key.chars();
    let capitalized = match c.next() {
        None => return "Body".to_string(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    };
    format!("{} (Body)", capitalized)
}

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

        let mut t = transform.compute_transform();
        t.scale = Vec3::splat(0.5);

        let spawned = commands
            .spawn((
                Name::new(body_display_name(&npc_name)),
                Body,
                RigidBody::Dynamic,
                Collider::capsule(prefab.radius * 0.5, prefab.height * 0.25),
                CollisionLayers::new(CollisionLayer::Prop, LayerMask::ALL),
                ColliderDensity(prefab.body.density),
                t,
            ))
            .with_child((
                Name::new("Body Model"),
                SceneRoot(assets.load(prefab.scene.clone())),
                Transform::from_rotation(prefab.body.model_rotation),
            ))
            .id();

        state.spawned.push((spawned, npc_name));
    }
}

const DESPAWN_Y: f32 = -1000.0;

fn respawn_fallen_bodies(
    mut commands: Commands,
    mut spawners: Query<(&BodySpawner, &GlobalTransform, &mut SpawnerState)>,
    transforms: Query<&GlobalTransform>,
    registry: Res<NpcRegistry>,
    assets: Res<AssetServer>,
) {
    for (_spawner, spawner_transform, mut state) in &mut spawners {
        let mut i = 0;
        while i < state.spawned.len() {
            let (entity, ref npc_name) = state.spawned[i];
            let should_respawn = match transforms.get(entity) {
                Ok(gt) => gt.translation().y < DESPAWN_Y,
                Err(_) => true,
            };

            if !should_respawn {
                i += 1;
                continue;
            }

            if transforms.get(entity).is_ok() {
                commands.entity(entity).despawn();
            }

            let Some(prefab) = registry.prefabs.get(npc_name) else {
                state.spawned.swap_remove(i);
                continue;
            };

            let mut t = spawner_transform.compute_transform();
            t.scale = Vec3::splat(0.5);

            let new_entity = commands
                .spawn((
                    Name::new(body_display_name(npc_name)),
                    Body,
                    RigidBody::Dynamic,
                    Collider::capsule(prefab.radius * 0.5, prefab.height * 0.25),
                    CollisionLayers::new(CollisionLayer::Prop, LayerMask::ALL),
                    ColliderDensity(prefab.body.density),
                    t,
                ))
                .with_child((
                    Name::new("Body Model"),
                    SceneRoot(assets.load(prefab.scene.clone())),
                    Transform::from_rotation(prefab.body.model_rotation),
                ))
                .id();

            state.spawned[i] = (new_entity, npc_name.clone());
            i += 1;
        }
    }
}

fn slot_bodies_in_graves(
    mut commands: Commands,
    sensors: Query<(&GraveSensor, &CollidingEntities, &Transform)>,
    mut graves: Query<&mut GraveState>,
    bodies: Query<Entity, (With<Body>, Without<Slotted>)>,
    parents: Query<&ChildOf>,
) {
    for (sensor, colliding, sensor_transform) in &sensors {
        let Ok(mut state) = graves.get_mut(sensor.0) else {
            continue;
        };

        for &colliding_entity in colliding.iter() {
            if state.filled >= state.slots {
                break;
            }

            let body_entity = std::iter::successors(Some(colliding_entity), |&e| {
                parents.get(e).ok().map(|p| p.0)
            })
            .find(|&e| bodies.get(e).is_ok());

            if let Some(body_entity) = body_entity {
                state.filled += 1;
                commands.entity(body_entity).insert((
                    Slotted,
                    RigidBody::Static,
                    GraveLerp {
                        target_y: sensor_transform.translation.y,
                    },
                ));
            }
        }
    }
}

const GRAVE_LERP_SPEED: f32 = 5.0;

fn lerp_slotted_bodies(
    mut commands: Commands,
    mut bodies: Query<(Entity, &mut Transform, &GraveLerp)>,
    time: Res<Time>,
) {
    for (entity, mut transform, lerp) in &mut bodies {
        let diff = lerp.target_y - transform.translation.y;
        if diff.abs() < 0.01 {
            transform.translation.y = lerp.target_y;
            commands.entity(entity).remove::<GraveLerp>();
        } else {
            transform.translation.y += diff * GRAVE_LERP_SPEED * time.delta_secs();
        }
    }
}

fn grave_reward(
    mut commands: Commands,
    mut graves: Query<(&mut GraveState, Option<&GraveVoxelVolume>)>,
    voxels: Query<&super::dig::VoxelSim>,
    mut crusts: ResMut<Crusts>,
) {
    for (mut state, voxel_volume) in &mut graves {
        if state.filled == 0 || state.filled == state.rewarded {
            continue;
        }
        let filled_enough = voxel_volume
            .and_then(|v| voxels.get(v.0).ok())
            .is_some_and(|sim| sim.air_ratio() <= 0.2);
        if filled_enough {
            let to_give = state.filled.saturating_sub(state.rewarded);
            crusts.add(to_give);
            state.rewarded += to_give;
            commands.trigger(super::crusts::CrustsRewarded(to_give));
        }
    }
}
