//! Plugin handling the player movement in particular.
//!
//! Note that this is separate from the `movement` module as that could be used
//! for other characters as well.

use animation::{PlayerAnimationState, setup_player_animations};
use avian3d::prelude::*;
use bevy::{ecs::entity::EntityHashSet, prelude::*};
use bevy_ahoy::prelude::*;
use bevy_landmass::{Character, prelude::*};

use bevy_trenchbroom::prelude::*;
use input::PlayerInputContext;
use navmesh_position::LastValidPlayerNavmeshPosition;

use std::any::TypeId;

use crate::{
    animation::AnimationState,
    asset_tracking::LoadResource,
    gameplay::tags::TagIndex,
    screens::Screen,
    third_party::{avian3d::CollisionLayer, bevy_trenchbroom::GetTrenchbroomModelPath as _},
};

/// Discrete player hit points. Starts at 3.
#[derive(Component)]
pub(crate) struct PlayerHealth {
    pub current: u32,
    pub max: u32,
}

/// While present on the player entity, the player cannot take damage.
/// Automatically removed when the timer expires.
#[derive(Component)]
pub(crate) struct Invincible(pub Timer);

/// Stored on the player entity so we can teleport back on fall-out.
#[derive(Component)]
struct SpawnPoint(Vec3);

/// Marker inserted when the player dies. Contains the respawn countdown timer.
#[derive(Component)]
pub(crate) struct PlayerDead(pub Timer);

mod animation;
pub(crate) mod assets;
pub(crate) mod camera;
pub(crate) mod dialogue;
pub(crate) mod input;
pub(crate) mod movement_sound;
pub(crate) mod navmesh_position;
pub(crate) mod pickup;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        animation::plugin,
        assets::plugin,
        camera::plugin,
        input::plugin,
        dialogue::plugin,
        movement_sound::plugin,
        pickup::plugin,
        navmesh_position::plugin,
    ));
    app.add_observer(setup_player);
    app.load_asset::<Gltf>(Player::model_path());
    app.add_systems(PreUpdate, assert_only_one_player);
    app.add_systems(
        Update,
        (
            push_props,
            tick_invincibility,
            respawn_fallen_player,
            detect_player_death,
            respawn_player,
        )
            .run_if(in_state(Screen::Gameplay)),
    );
}

#[point_class(
    base(Transform, Visibility),
    model("models/view_model/view_model.gltf")
)]
pub(crate) struct Player;

/// The radius of the player character's capsule.
pub(crate) const PLAYER_RADIUS: f32 = 0.5;
const PLAYER_HEIGHT: f32 = 1.8;

/// The half height of the player character's capsule is the distance between the character's center and the lowest point of its collider.
const PLAYER_HALF_HEIGHT: f32 = PLAYER_HEIGHT / 2.0;

/// The height used for the player's floating character controller.
///
/// Such a controller works by keeping the character itself at a more-or-less constant height above the ground by
/// using a spring. It's important to make sure that this floating height is greater (even if by little) than the half height.
///
/// In this case, we use 30 cm of padding to make the player float nicely up stairs.
const PLAYER_FLOAT_HEIGHT: f32 = PLAYER_HALF_HEIGHT + 0.01;

fn setup_player(
    add: On<Add, Player>,
    mut commands: Commands,
    archipelago: Single<Entity, With<Archipelago3d>>,
    transforms: Query<&Transform>,
) {
    let spawn_pos = transforms
        .get(add.entity)
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);

    let mut self_hashset = EntityHashSet::new();
    self_hashset.insert(add.entity);
    let filter = SpatialQueryFilter {
        mask: [CollisionLayer::Level].into(),
        excluded_entities: self_hashset.clone(),
    };

    commands
        .entity(add.entity)
        .insert((
            RigidBody::Kinematic,
            PlayerInputContext,
            Collider::cylinder(PLAYER_RADIUS, PLAYER_HEIGHT),
            CharacterController {
                jump_height: 3.5,
                filter: filter,
                acceleration_hz: 10.0,
                friction_hz: 30.0,
                ..default()
            },
            ColliderDensity(1_000.0),
            CollisionLayers::new(CollisionLayer::Character, CollisionLayer::Level),
            AnimationState::<PlayerAnimationState>::default(),
            PlayerHealth { current: 3, max: 3 },
            SpawnPoint(spawn_pos),
            children![(
                Name::new("Player Landmass Character"),
                Transform::from_xyz(0.0, -PLAYER_FLOAT_HEIGHT, 0.0),
                Character3dBundle {
                    character: Character::default(),
                    settings: CharacterSettings {
                        radius: PLAYER_RADIUS,
                    },
                    archipelago_ref: ArchipelagoRef3d::new(*archipelago),
                },
                LastValidPlayerNavmeshPosition::default(),
            )],
        ))
        .observe(setup_player_animations);
}

fn assert_only_one_player(player: Populated<(), With<Player>>) {
    assert_eq!(1, player.iter().count());
}

const PROP_PUSH_SPEED: f32 = 5.0;

fn push_props(
    player: Single<(&GlobalTransform, &Collider), With<Player>>,
    spatial_query: SpatialQuery,
    mut props: Query<(&GlobalTransform, &mut LinearVelocity)>,
) {
    let (player_transform, player_collider) = player.into_inner();
    let player_pos = player_transform.translation();

    let hits = spatial_query.shape_intersections(
        player_collider,
        player_pos,
        player_transform.to_isometry().rotation,
        &SpatialQueryFilter::from_mask(CollisionLayer::Prop),
    );

    for entity in hits {
        let Ok((prop_transform, mut velocity)) = props.get_mut(entity) else {
            continue;
        };
        let prop_pos = prop_transform.translation();
        let delta = prop_pos - player_pos;
        let horizontal = Vec3::new(delta.x, 0.0, delta.z);
        let distance = horizontal.length();

        if distance < 0.001 {
            continue;
        }

        let direction = horizontal / distance;
        let strength = (1.0 - (distance / PLAYER_RADIUS).min(1.0)) * PROP_PUSH_SPEED;
        velocity.x = direction.x * strength;
        velocity.z = direction.z * strength;
    }
}

fn tick_invincibility(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Invincible), With<Player>>,
) {
    for (entity, mut inv) in &mut query {
        inv.0.tick(time.delta());
        if inv.0.just_finished() {
            commands.entity(entity).remove::<Invincible>();
        }
    }
}

const DESPAWN_Y: f32 = -1000.0;

fn respawn_fallen_player(mut player: Query<(&mut Transform, &SpawnPoint), With<Player>>) {
    for (mut transform, spawn) in &mut player {
        if transform.translation.y < DESPAWN_Y {
            transform.translation = spawn.0;
        }
    }
}

/// Try to deal 1 HP of damage to the player. Returns `true` if damage was applied.
/// Grants 1 second of invincibility on hit.
pub(crate) fn hurt_player(
    commands: &mut Commands,
    entity: Entity,
    health: &mut PlayerHealth,
    invincible: Option<&Invincible>,
) -> bool {
    if invincible.is_some() {
        return false;
    }
    health.current = health.current.saturating_sub(1);
    commands
        .entity(entity)
        .insert(Invincible(Timer::from_seconds(1.0, TimerMode::Once)));
    true
}

const RESPAWN_SECONDS: f32 = 3.0;

fn detect_player_death(
    mut commands: Commands,
    player: Query<(Entity, &PlayerHealth), (With<Player>, Without<PlayerDead>)>,
    mut blocks_input: ResMut<input::BlocksInput>,
) {
    let Ok((entity, health)) = player.single() else {
        return;
    };
    if health.current == 0 {
        commands.entity(entity).insert(PlayerDead(Timer::from_seconds(
            RESPAWN_SECONDS,
            TimerMode::Once,
        )));
        blocks_input.insert(TypeId::of::<PlayerDead>());
    }
}

fn respawn_player(
    mut commands: Commands,
    time: Res<Time>,
    mut player: Query<
        (Entity, &mut PlayerDead, &mut PlayerHealth, &SpawnPoint, &mut Transform),
        With<Player>,
    >,
    tag_index: Res<TagIndex>,
    global_transforms: Query<&GlobalTransform>,
    mut blocks_input: ResMut<input::BlocksInput>,
) {
    let Ok((entity, mut dead, mut health, spawn_point, mut transform)) = player.single_mut()
    else {
        return;
    };

    dead.0.tick(time.delta());
    if !dead.0.just_finished() {
        return;
    }

    // Find checkpoint tagged "tutorial_spawn", fall back to SpawnPoint.
    let respawn_pos = tag_index
        .get("tutorial_spawn")
        .and_then(|entities| {
            entities
                .iter()
                .next()
                .and_then(|&e| global_transforms.get(e).ok())
        })
        .map(|tf| tf.translation())
        .unwrap_or(spawn_point.0);

    transform.translation = respawn_pos;
    health.current = health.max;
    commands.entity(entity).remove::<(PlayerDead, Invincible)>();
    blocks_input.remove(&TypeId::of::<PlayerDead>());
}
