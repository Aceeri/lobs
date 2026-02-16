//! NPC spawning, death, and related systems.

use avian3d::prelude::*;
use bevy::{ecs::entity::EntityHashSet, prelude::*};

use bevy_ahoy::CharacterController;
use bevy_trenchbroom::prelude::*;

use bevy::platform::collections::HashMap;

use crate::{
    asset_tracking::LoadResource,
    third_party::{
        avian3d::CollisionLayer,
        bevy_trenchbroom::{GetTrenchbroomModelPath, LoadTrenchbroomModel as _},
        bevy_yarnspinner::YarnNode,
    },
};

pub(crate) mod ai;
mod animation;
mod assets;
pub(super) mod shooting;
mod sound;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        ai::plugin,
        animation::plugin,
        assets::plugin,
        shooting::plugin,
        sound::plugin,
    ));
    app.load_asset::<Gltf>(Npc::model_path());
    app.load_asset::<Gltf>("models/crab/scene.gltf");
    app.load_asset::<Gltf>("models/Shark.glb");
    app.load_asset::<Gltf>("models/Whale.glb");
    app.load_asset::<Gltf>("models/Turtle.glb");
    app.load_asset::<Gltf>("models/Seal.glb");
    app.load_asset::<Gltf>("models/Octopus.glb");
    app.load_asset::<Gltf>("models/tommy_gun.glb");
    app.add_observer(on_add);
    app.add_observer(on_add_enemy_gunner);
    app.add_observer(on_npc_aggro);
    app.add_observer(on_npc_death);
    app.add_observer(init_npc_spawner);
    app.add_observer(on_spawn_npc);
    app.add_observer(init_enemy_spawner);
    app.add_observer(on_spawn_enemy);
    app.add_systems(
        Update,
        (respawn_fallen_npcs, respawn_fallen_enemies, unparent_npcs),
    );
    app.init_resource::<NpcRegistry>();
}

#[derive(Component)]
pub(crate) struct NpcDead;

#[derive(Component)]
pub(crate) struct NpcAggro;

#[derive(Component)]
struct NpcAggroGun;

#[derive(Component)]
struct GunOffset(Vec3);

#[derive(Component, Clone)]
pub(crate) struct BodyConfig {
    pub collider: ColliderConstructor,
    pub model_rotation: Quat,
    pub density: f32,
}

impl Default for BodyConfig {
    fn default() -> Self {
        Self {
            collider: ColliderConstructor::ConvexHullFromMesh,
            model_rotation: Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
            density: 1000.0,
        }
    }
}

#[derive(Clone)]
pub(crate) struct NpcPrefab {
    pub scene: String,
    pub radius: f32,
    pub height: f32,
    pub body: BodyConfig,
    pub gun_offset: Vec3,
}

const DEFAULT_GUN_OFFSET: Vec3 = Vec3::new(0.7, 0.3, 0.7);

#[derive(Resource)]
pub(crate) struct NpcRegistry {
    pub prefabs: HashMap<String, NpcPrefab>,
}

impl Default for NpcRegistry {
    fn default() -> Self {
        let mut prefabs = HashMap::new();
        prefabs.insert(
            "lobster".into(),
            NpcPrefab {
                scene: Npc::scene_path(),
                radius: NPC_RADIUS,
                height: NPC_HEIGHT,
                body: BodyConfig::default(),
                gun_offset: DEFAULT_GUN_OFFSET,
            },
        );
        prefabs.insert(
            "crab".into(),
            NpcPrefab {
                scene: "models/crab/scene.gltf#Scene0".into(),
                radius: 0.5,
                height: 0.8,
                body: BodyConfig::default(),
                gun_offset: DEFAULT_GUN_OFFSET,
            },
        );
        prefabs.insert(
            "shark".into(),
            NpcPrefab {
                scene: "models/Shark.glb#Scene0".into(),
                radius: NPC_RADIUS,
                height: NPC_HEIGHT,
                body: BodyConfig::default(),
                gun_offset: DEFAULT_GUN_OFFSET,
            },
        );
        prefabs.insert(
            "whale".into(),
            NpcPrefab {
                scene: "models/Whale.glb#Scene0".into(),
                radius: NPC_RADIUS,
                height: NPC_HEIGHT,
                body: BodyConfig::default(),
                gun_offset: DEFAULT_GUN_OFFSET,
            },
        );
        prefabs.insert(
            "turtle".into(),
            NpcPrefab {
                scene: "models/Turtle.glb#Scene0".into(),
                radius: NPC_RADIUS,
                height: NPC_HEIGHT,
                body: BodyConfig::default(),
                gun_offset: DEFAULT_GUN_OFFSET,
            },
        );
        prefabs.insert(
            "seal".into(),
            NpcPrefab {
                scene: "models/Seal.glb#Scene0".into(),
                radius: NPC_RADIUS,
                height: NPC_HEIGHT,
                body: BodyConfig::default(),
                gun_offset: DEFAULT_GUN_OFFSET,
            },
        );
        prefabs.insert(
            "octopus".into(),
            NpcPrefab {
                scene: "models/Octopus.glb#Scene0".into(),
                radius: NPC_RADIUS,
                height: NPC_HEIGHT,
                body: BodyConfig {
                    model_rotation: Quat::IDENTITY,
                    ..BodyConfig::default()
                },
                gun_offset: DEFAULT_GUN_OFFSET,
            },
        );
        Self { prefabs }
    }
}

// #[point_class(base(Transform, Visibility), model("models/fox/Fox.gltf"))]
#[point_class(
    base(Transform, Visibility),
    model("models/lobster/lowpoly_lobster.glb")
)]
pub(crate) struct Npc {
    pub tag: String,
    pub yarn_node: String,
    pub model: String,
    pub health: f32,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            tag: String::new(),
            yarn_node: String::new(),
            model: String::new(),
            health: 0.0,
        }
    }
}

#[point_class(
    base(Transform, Visibility),
    model("models/lobster/lowpoly_lobster.glb")
)]
pub(crate) struct EnemyGunner {
    /// Comma-separated tags for identification/objectives.
    pub tag: String,
    /// Registry key for the model prefab (e.g. "lobster", "shark").
    pub model: String,
    /// Starting health. 0 = use default.
    pub health: f32,
    /// Firing pattern: "radial", "spread", etc.
    pub pattern: String,
    /// Shots per second.
    pub fire_rate: f32,
    /// Projectile travel speed.
    pub projectile_speed: f32,
    /// Projectiles per burst.
    pub projectile_count: u32,
    /// Aggro/firing range.
    pub range: f32,
    /// Tag to auto-target (e.g. "larry"). Empty = target player.
    pub target_tag: String,
    /// Radius for player proximity aggro swap.
    pub aggro_radius: f32,
}

impl Default for EnemyGunner {
    fn default() -> Self {
        Self {
            tag: String::new(),
            model: String::new(),
            health: 0.0,
            pattern: "radial".into(),
            fire_rate: 1.5,
            projectile_speed: 5.0,
            projectile_count: 12,
            range: 20.0,
            target_tag: String::new(),
            aggro_radius: 15.0,
        }
    }
}

pub(crate) use super::tags::Tags;

#[derive(Component)]
pub(crate) struct Body;

#[derive(Component)]
pub(crate) struct Health(pub f32);

pub(crate) const NPC_RADIUS: f32 = 0.6;
pub(crate) const NPC_HEIGHT: f32 = 1.3;
const NPC_HALF_HEIGHT: f32 = NPC_HEIGHT / 2.0;
const NPC_FLOAT_HEIGHT: f32 = NPC_HALF_HEIGHT + 0.01;
const NPC_SPEED: f32 = 7.0;
const DEFAULT_NPC_HEALTH: f32 = 100.0;

fn npc_display_name(model_key: &str, kind: &str, tags: &Tags) -> String {
    let model = if model_key.is_empty() {
        "lobster"
    } else {
        model_key
    };
    let mut parts: Vec<&str> = Vec::new();
    if !kind.is_empty() {
        parts.push(kind);
    }
    for tag in &tags.0 {
        parts.push(tag.as_str());
    }
    let capitalized = {
        let mut c = model.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().to_string() + c.as_str(),
        }
    };
    if parts.is_empty() {
        capitalized
    } else {
        format!("{} ({})", capitalized, parts.join(", "))
    }
}

fn on_add(
    add: On<Add, Npc>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    npcs: Query<&Npc>,
    registry: Res<NpcRegistry>,
) {
    let npc = npcs.get(add.entity).ok();
    let npc_tags = npc
        .map(|npc| Tags::from_csv(&npc.tag))
        .unwrap_or(Tags(Vec::new()));
    let yarn_node = npc
        .map(|npc| npc.yarn_node.trim().to_string())
        .unwrap_or_default();
    let model_key = npc
        .map(|npc| npc.model.trim().to_string())
        .unwrap_or_default();
    let health = npc
        .map(|npc| {
            if npc.health > 0.0 {
                npc.health
            } else {
                DEFAULT_NPC_HEALTH
            }
        })
        .unwrap_or(DEFAULT_NPC_HEALTH);

    let prefab = if !model_key.is_empty() {
        registry.prefabs.get(&model_key)
    } else {
        None
    };

    let mut self_hashset = EntityHashSet::new();
    self_hashset.insert(add.entity);
    let filter = SpatialQueryFilter {
        mask: [CollisionLayer::Level, CollisionLayer::Prop].into(),
        excluded_entities: self_hashset.clone(),
    };

    let body_config = prefab.map(|p| p.body.clone()).unwrap_or_default();
    let gun_offset = prefab.map(|p| p.gun_offset).unwrap_or(DEFAULT_GUN_OFFSET);

    let display_name = npc_display_name(&model_key, "", &npc_tags);

    let mut entity_commands = commands.entity(add.entity);
    entity_commands.insert((
        Name::new(display_name),
        Collider::cylinder(NPC_RADIUS, NPC_HEIGHT),
        CharacterController {
            speed: NPC_SPEED,
            filter: filter,
            ..default()
        },
        ColliderDensity(1_000.0),
        RigidBody::Kinematic,
        CollisionLayers::new(
            CollisionLayer::Character,
            [CollisionLayer::Level, CollisionLayer::Prop],
        ),
        Health(health),
        body_config.clone(),
        GunOffset(gun_offset),
        npc_tags.clone(),
    ));

    if !yarn_node.is_empty() {
        entity_commands.insert(YarnNode::new(&yarn_node));
    }

    let (scene, rotation) = if let Some(prefab) = prefab {
        (assets.load(&prefab.scene), prefab.body.model_rotation)
    } else {
        (
            assets.load_trenchbroom_model::<Npc>(),
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        )
    };

    entity_commands.with_child((
        Name::new("Npc Model"),
        SceneRoot(scene),
        Transform::from_xyz(0.0, 0.0, 0.0).with_rotation(rotation),
    ));
}

fn on_add_enemy_gunner(
    add: On<Add, EnemyGunner>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    gunners: Query<&EnemyGunner>,
    registry: Res<NpcRegistry>,
) {
    let entity = add.entity;
    let gunner = gunners.get(entity).ok();
    let npc_tags = gunner
        .map(|g| Tags::from_csv(&g.tag))
        .unwrap_or(Tags(Vec::new()));
    let model_key = gunner
        .map(|g| g.model.trim().to_string())
        .unwrap_or_default();
    let health = gunner
        .map(|g| {
            if g.health > 0.0 {
                g.health
            } else {
                DEFAULT_NPC_HEALTH
            }
        })
        .unwrap_or(DEFAULT_NPC_HEALTH);

    let prefab = if !model_key.is_empty() {
        registry.prefabs.get(&model_key)
    } else {
        None
    };

    let shooter = gunner
        .map(|g| shooting::NpcShooter::from_gunner(g))
        .unwrap_or_default();

    let mut self_hashset = EntityHashSet::new();
    self_hashset.insert(entity);
    let filter = SpatialQueryFilter {
        mask: [CollisionLayer::Level, CollisionLayer::Prop].into(),
        excluded_entities: self_hashset,
    };

    let body_config = prefab.map(|p| p.body.clone()).unwrap_or_default();
    let gun_offset = prefab.map(|p| p.gun_offset).unwrap_or(DEFAULT_GUN_OFFSET);

    let display_name = npc_display_name(&model_key, "Gunner", &npc_tags);

    let aggro_config = gunner
        .map(|g| shooting::AggroConfig {
            target_tag: g.target_tag.trim().to_string(),
            aggro_radius: g.aggro_radius,
            swapped_to_player: false,
        })
        .unwrap_or(shooting::AggroConfig {
            target_tag: String::new(),
            aggro_radius: 15.0,
            swapped_to_player: false,
        });

    commands.entity(entity).insert((
        Name::new(display_name),
        Collider::cylinder(NPC_RADIUS, NPC_HEIGHT),
        CharacterController {
            speed: NPC_SPEED,
            filter,
            ..default()
        },
        ColliderDensity(1_000.0),
        RigidBody::Kinematic,
        CollisionLayers::new(
            CollisionLayer::Character,
            [CollisionLayer::Level, CollisionLayer::Prop],
        ),
        Health(health),
        body_config.clone(),
        GunOffset(gun_offset),
        NpcAggro,
        shooter,
        aggro_config,
        npc_tags,
    ));

    let (scene, rotation) = if let Some(prefab) = prefab {
        (assets.load(&prefab.scene), prefab.body.model_rotation)
    } else {
        (
            assets.load_trenchbroom_model::<EnemyGunner>(),
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        )
    };

    commands.entity(entity).with_child((
        Name::new("Npc Model"),
        SceneRoot(scene),
        Transform::from_xyz(0.0, 0.0, 0.0).with_rotation(rotation),
    ));
}

fn on_npc_aggro(
    aggro: On<Add, NpcAggro>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    gun_offsets: Query<&GunOffset>,
) {
    let entity = aggro.entity;
    let offset = gun_offsets
        .get(entity)
        .map(|g| g.0)
        .unwrap_or(DEFAULT_GUN_OFFSET);

    commands.entity(entity).with_child((
        Name::new("Aggro Gun"),
        NpcAggroGun,
        SceneRoot(assets.load("models/tommy_gun.glb#Scene0")),
        Transform::from_translation(offset)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(0.01)),
    ));
}

fn on_npc_death(
    add: On<Add, NpcDead>,
    mut commands: Commands,
    npc_entity: Query<(Entity, &Transform, Option<&BodyConfig>, Option<&Name>)>,
    children: Query<&Children>,
    agents: Query<(), With<ai::WantsToFollowPlayer>>,
    aggro_guns: Query<(), With<NpcAggroGun>>,
) {
    let Ok((entity, transform, body_config, name)) = npc_entity.get(add.entity) else {
        warn!("npc death didnt have transform");
        return;
    };
    let default_config = BodyConfig::default();
    let config = body_config.unwrap_or(&default_config);

    let dead_name = match name {
        Some(n) => {
            let s = n.as_str();
            if let Some(paren) = s.rfind(')') {
                format!("{}, Dead)", &s[..paren])
            } else {
                format!("{} (Dead)", s)
            }
        }
        None => "Unknown (Dead)".to_string(),
    };

    commands
        .entity(entity)
        .remove::<(
            Npc,
            EnemyGunner,
            /* cc */
            CharacterController,
            bevy_ahoy::input::AccumulatedInput,
            bevy_ahoy::CharacterControllerState,
            bevy_ahoy::CharacterControllerOutput,
            bevy_ahoy::CharacterControllerDerivedProps,
            bevy_ahoy::prelude::WaterState,
            CustomPositionIntegration,
            /* other */
            Health,
            YarnNode,
            shooting::NpcShooter,
            shooting::EnemyAlert,
            shooting::AggroTarget,
            shooting::AggroConfig,
        )>()
        .insert((
            Name::new(dead_name),
            RigidBody::Dynamic,
            Body,
            transform.with_scale(Vec3::splat(0.75)),
            Collider::capsule(NPC_RADIUS, NPC_HEIGHT),
            CollisionLayers::new(
                [CollisionLayer::Prop, CollisionLayer::Ragdoll],
                LayerMask::ALL,
            ),
            ColliderDensity(config.density),
            LinearVelocity(Vec3::ZERO),
            AngularVelocity(Vec3::ZERO),
        ));

    if let Ok(children) = children.get(entity) {
        for child in children.iter() {
            if agents.get(child).is_ok() || aggro_guns.get(child).is_ok() {
                commands.entity(child).despawn();
            }
        }
    }
}

fn unparent_npcs(
    mut commands: Commands,
    npcs: Query<Entity, (With<ChildOf>, Or<(Added<Npc>, Added<EnemyGunner>)>)>,
) {
    for entity in &npcs {
        commands.entity(entity).remove::<ChildOf>();
    }
}

#[point_class(base(Transform, Visibility))]
pub(crate) struct NpcSpawner {
    /// Unique name to target this spawner from events.
    pub name: String,
    /// Comma-separated tags applied to spawned NPCs.
    pub tag: String,
    /// Default model prefab key when queue is empty.
    pub model: String,
    /// Comma-separated model keys to cycle through on each spawn.
    pub queue: String,
}

impl Default for NpcSpawner {
    fn default() -> Self {
        Self {
            name: String::new(),
            tag: String::new(),
            model: String::new(),
            queue: String::new(),
        }
    }
}

#[derive(Component)]
struct NpcSpawnerState {
    queue: Vec<String>,
    index: usize,
    spawned: Vec<(Entity, String)>,
}

fn init_npc_spawner(
    add: On<Add, NpcSpawner>,
    mut commands: Commands,
    spawners: Query<&NpcSpawner>,
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
    commands.entity(add.entity).insert(NpcSpawnerState {
        queue,
        index: 0,
        spawned: Vec::new(),
    });
}

#[derive(Default, Clone)]
pub(crate) struct NpcOverrides {
    pub health: Option<f32>,
    pub tag: Option<String>,
    pub yarn_node: Option<String>,
}

#[derive(Event)]
pub(crate) enum SpawnNpc {
    Queue {
        spawner_name: String,
        overrides: NpcOverrides,
    },
    Direct {
        spawner_name: String,
        model: String,
        overrides: NpcOverrides,
    },
}

fn on_spawn_npc(
    event: On<SpawnNpc>,
    mut commands: Commands,
    mut spawners: Query<(&NpcSpawner, &GlobalTransform, &mut NpcSpawnerState)>,
) {
    let (target_spawner, target_model, overrides): (&str, Option<&str>, &NpcOverrides) =
        match &*event {
            SpawnNpc::Queue {
                spawner_name,
                overrides,
            } => (spawner_name.as_str(), None, overrides),
            SpawnNpc::Direct {
                spawner_name,
                model,
                overrides,
            } => (spawner_name.as_str(), Some(model.as_str()), overrides),
        };

    for (spawner, transform, mut state) in &mut spawners {
        if spawner.name != target_spawner {
            continue;
        }

        let model_key = match target_model {
            Some(m) => m.to_string(),
            None => {
                if state.queue.is_empty() {
                    spawner.model.clone()
                } else {
                    let name = state.queue[state.index].clone();
                    state.index = (state.index + 1) % state.queue.len();
                    name
                }
            }
        };

        let t = transform.compute_transform();
        let tag = overrides.tag.clone().unwrap_or_else(|| spawner.tag.clone());

        let spawned = commands
            .spawn((
                Npc {
                    tag: tag.clone(),
                    yarn_node: overrides.yarn_node.clone().unwrap_or_default(),
                    model: model_key.clone(),
                    health: overrides.health.unwrap_or(0.0),
                },
                t,
                Visibility::default(),
                Tags::from_csv(&tag),
            ))
            .id();

        state.spawned.push((spawned, model_key));
    }
}

const DESPAWN_Y: f32 = -1000.0;

fn respawn_fallen_npcs(
    mut commands: Commands,
    mut spawners: Query<(&NpcSpawner, &GlobalTransform, &mut NpcSpawnerState)>,
    transforms: Query<&GlobalTransform>,
) {
    for (spawner, spawner_transform, mut state) in &mut spawners {
        let mut i = 0;
        while i < state.spawned.len() {
            let (entity, ref model_key) = state.spawned[i];
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

            let t = spawner_transform.compute_transform();
            let tag = spawner.tag.clone();

            let new_entity = commands
                .spawn((
                    Npc {
                        tag,
                        yarn_node: String::new(),
                        model: model_key.clone(),
                        health: 0.0,
                    },
                    t,
                    Visibility::default(),
                    Tags::from_csv(&spawner.tag),
                ))
                .id();

            state.spawned[i] = (new_entity, model_key.clone());
            i += 1;
        }
    }
}

#[point_class(base(Transform, Visibility))]
pub(crate) struct EnemySpawner {
    /// Unique name to target this spawner from events.
    pub name: String,
    /// Comma-separated tags applied to spawned enemies.
    pub tag: String,
    /// Default model prefab key when queue is empty.
    pub model: String,
    /// Comma-separated model keys to cycle through on each spawn.
    pub queue: String,
    /// Firing pattern passed to spawned EnemyGunners.
    pub pattern: String,
    /// Shots per second for spawned enemies.
    pub fire_rate: f32,
    /// Projectile travel speed for spawned enemies.
    pub projectile_speed: f32,
    /// Projectiles per burst for spawned enemies.
    pub projectile_count: u32,
    /// Aggro/firing range for spawned enemies.
    pub range: f32,
    /// Tag to auto-target for spawned enemies. Empty = target player.
    pub target_tag: String,
    /// Radius for player proximity aggro swap for spawned enemies.
    pub aggro_radius: f32,
}

impl Default for EnemySpawner {
    fn default() -> Self {
        Self {
            name: String::new(),
            tag: String::new(),
            model: String::new(),
            queue: String::new(),
            pattern: "radial".into(),
            fire_rate: 1.5,
            projectile_speed: 5.0,
            projectile_count: 12,
            range: 20.0,
            target_tag: String::new(),
            aggro_radius: 15.0,
        }
    }
}

#[derive(Component)]
struct EnemySpawnerState {
    queue: Vec<String>,
    index: usize,
    spawned: Vec<(Entity, String)>,
}

fn init_enemy_spawner(
    add: On<Add, EnemySpawner>,
    mut commands: Commands,
    spawners: Query<&EnemySpawner>,
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
    commands.entity(add.entity).insert(EnemySpawnerState {
        queue,
        index: 0,
        spawned: Vec::new(),
    });
}

#[derive(Event)]
pub(crate) enum SpawnEnemy {
    Queue { spawner_name: String },
    Direct { spawner_name: String, model: String },
}

fn on_spawn_enemy(
    event: On<SpawnEnemy>,
    mut commands: Commands,
    mut spawners: Query<(&EnemySpawner, &GlobalTransform, &mut EnemySpawnerState)>,
) {
    let (target_spawner, target_model): (&str, Option<&str>) = match &*event {
        SpawnEnemy::Queue { spawner_name } => (spawner_name.as_str(), None),
        SpawnEnemy::Direct {
            spawner_name,
            model,
        } => (spawner_name.as_str(), Some(model.as_str())),
    };

    for (spawner, transform, mut state) in &mut spawners {
        if spawner.name != target_spawner {
            continue;
        }

        let model_key = match target_model {
            Some(m) => m.to_string(),
            None => {
                if state.queue.is_empty() {
                    spawner.model.clone()
                } else {
                    let name = state.queue[state.index].clone();
                    state.index = (state.index + 1) % state.queue.len();
                    name
                }
            }
        };

        let t = transform.compute_transform();

        let spawned = commands
            .spawn((
                EnemyGunner {
                    tag: spawner.tag.clone(),
                    model: model_key.clone(),
                    health: 0.0,
                    pattern: spawner.pattern.clone(),
                    fire_rate: spawner.fire_rate,
                    projectile_speed: spawner.projectile_speed,
                    projectile_count: spawner.projectile_count,
                    range: spawner.range,
                    target_tag: spawner.target_tag.clone(),
                    aggro_radius: spawner.aggro_radius,
                },
                t,
                Visibility::default(),
            ))
            .id();

        state.spawned.push((spawned, model_key));
    }
}

fn respawn_fallen_enemies(
    mut commands: Commands,
    mut spawners: Query<(&EnemySpawner, &GlobalTransform, &mut EnemySpawnerState)>,
    transforms: Query<&GlobalTransform>,
) {
    for (spawner, spawner_transform, mut state) in &mut spawners {
        let mut i = 0;
        while i < state.spawned.len() {
            let (entity, ref model_key) = state.spawned[i];
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

            let t = spawner_transform.compute_transform();

            let new_entity = commands
                .spawn((
                    EnemyGunner {
                        tag: spawner.tag.clone(),
                        model: model_key.clone(),
                        health: 0.0,
                        pattern: spawner.pattern.clone(),
                        fire_rate: spawner.fire_rate,
                        projectile_speed: spawner.projectile_speed,
                        projectile_count: spawner.projectile_count,
                        range: spawner.range,
                        target_tag: spawner.target_tag.clone(),
                        aggro_radius: spawner.aggro_radius,
                    },
                    t,
                    Visibility::default(),
                ))
                .id();

            state.spawned[i] = (new_entity, model_key.clone());
            i += 1;
        }
    }
}
