//! Enemy projectile system — bullet-hell style slow-moving orbs.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_seedling::prelude::*;
use bevy_seedling::sample::AudioSample;
use std::f32::consts::{PI, TAU};

use crate::{
    audio::SpatialPool,
    gameplay::{
        player::{Invincible, Player, PlayerHealth, hurt_player},
        tags::TagIndex,
    },
    screens::Screen,
    third_party::avian3d::CollisionLayer,
};

use super::{EnemyGunner, Health, NpcAggro, NpcDead};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        (
            resolve_aggro_targets,
            aggro_swap,
            enemy_detection,
            rotate_alert_enemies,
            npc_shoot,
            move_projectiles,
            projectile_hit_player,
            projectile_hit_npc,
            projectile_hit_level,
        )
            .chain()
            .run_if(in_state(Screen::Gameplay)),
    );
    app.add_observer(init_projectile_assets);
}


#[derive(Resource)]
struct ProjectileAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    gunshot: Handle<AudioSample>,
}

fn init_projectile_assets(
    _add: On<Add, Player>, // initialize once when the player spawns
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<ProjectileAssets>>,
) {
    if existing.is_some() {
        return;
    }
    commands.insert_resource(ProjectileAssets {
        mesh: meshes.add(Sphere::new(0.1)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.3, 0.05),
            emissive: LinearRgba::new(6.0, 1.5, 0.2, 1.0),
            unlit: true,
            ..default()
        }),
        gunshot: asset_server.load("audio/sound_effects/smg_shot.ogg"),
    });
}


#[derive(Component, Clone, Debug)]
pub(crate) struct Faction(pub String);

impl Faction {
    /// Returns true if a projectile from `self` faction is allowed to hurt `target` faction.
    pub fn can_hurt(&self, target: &Faction) -> bool {
        match (self.0.as_str(), target.0.as_str()) {
            // Player can hurt everyone
            ("player", _) => true,
            // Lobster (larry) shouldn't hurt the player
            ("lobster", "player") => false,
            // Enemies shouldn't hurt other enemies
            ("enemy", "enemy") => false,
            // Everything else is fair game
            _ => true,
        }
    }
}

#[derive(Component)]
pub(crate) struct EnemyProjectile;

#[derive(Component)]
struct Projectile {
    velocity: Vec3,
    lifetime: Timer,
}

#[derive(Component)]
pub(crate) struct NpcShooter {
    pattern: FiringPattern,
    fire_rate: Timer,
    range: f32,
    projectile_speed: f32,
    projectile_count: u32,
}

impl Default for NpcShooter {
    fn default() -> Self {
        Self {
            pattern: FiringPattern::RadialBurst,
            fire_rate: Timer::from_seconds(1.5, TimerMode::Repeating),
            range: 20.0,
            projectile_speed: 5.0,
            projectile_count: 12,
        }
    }
}

impl NpcShooter {
    pub fn from_gunner(g: &EnemyGunner) -> Self {
        let pattern = match g.pattern.as_str() {
            "spread" => FiringPattern::AimedSpread,
            _ => FiringPattern::RadialBurst,
        };
        Self {
            pattern,
            fire_rate: Timer::from_seconds(g.fire_rate, TimerMode::Repeating),
            range: g.range,
            projectile_speed: g.projectile_speed,
            projectile_count: g.projectile_count,
        }
    }
}

enum FiringPattern {
    RadialBurst,
    AimedSpread,
}

/// Tracks that an enemy has detected the player and is actively engaging.
#[derive(Component)]
pub(crate) struct EnemyAlert {
    last_seen_position: Vec3,
    /// Counts down after losing sight; enemy stays alert briefly.
    lose_sight_timer: Timer,
}

#[derive(Component)]
pub(crate) struct AggroTarget(pub Entity);

#[derive(Component)]
pub(crate) struct AggroConfig {
    pub target_tag: String,
    pub aggro_radius: f32,
    pub swapped_to_player: bool,
}


const PROJECTILE_LIFETIME: f32 = 6.0;
const SPREAD_HALF_ANGLE: f32 = PI / 6.0; // 30 degrees total cone
/// Half of the 120° FOV detection cone (in radians).
const DETECTION_HALF_ANGLE: f32 = PI / 3.0; // 60°
/// How long an enemy stays alert after losing sight of the player.
const LOSE_SIGHT_DURATION: f32 = 3.0;


fn resolve_aggro_targets(
    mut commands: Commands,
    tag_index: Res<TagIndex>,
    mut enemies: Query<
        (Entity, &mut AggroConfig),
        (With<NpcAggro>, Without<AggroTarget>),
    >,
    dead: Query<(), With<NpcDead>>,
    player: Option<Single<Entity, With<Player>>>,
) {
    let Some(player) = player else { return };
    let player_entity = *player;

    for (entity, mut config) in &mut enemies {
        if config.target_tag.is_empty() {
            commands.entity(entity).insert(AggroTarget(player_entity));
            config.swapped_to_player = true;
            continue;
        }

        let target = tag_index
            .get(&config.target_tag)
            .and_then(|set| set.iter().find(|e| dead.get(**e).is_err()))
            .copied();

        match target {
            Some(t) => {
                commands.entity(entity).insert(AggroTarget(t));
            }
            None => {
                commands.entity(entity).insert(AggroTarget(player_entity));
                config.swapped_to_player = true;
            }
        }
    }
}

fn aggro_swap(
    mut enemies: Query<(&GlobalTransform, &mut AggroTarget, &mut AggroConfig), With<NpcAggro>>,
    player: Option<Single<(Entity, &GlobalTransform), With<Player>>>,
    dead: Query<(), With<NpcDead>>,
) {
    let Some(player) = player else { return };
    let (player_entity, player_transform) = *player;
    let player_pos = player_transform.translation();

    for (npc_transform, mut target, mut config) in &mut enemies {
        if config.swapped_to_player {
            continue;
        }

        if dead.get(target.0).is_ok() {
            target.0 = player_entity;
            config.swapped_to_player = true;
            continue;
        }

        let distance = npc_transform.translation().distance(player_pos);
        if distance < config.aggro_radius {
            target.0 = player_entity;
            config.swapped_to_player = true;
        }
    }
}

fn enemy_detection(
    mut commands: Commands,
    time: Res<Time>,
    spatial_query: SpatialQuery,
    mut enemies: Query<
        (
            Entity,
            &NpcShooter,
            &GlobalTransform,
            Option<&AggroTarget>,
            Option<&mut EnemyAlert>,
        ),
        With<NpcAggro>,
    >,
    player: Option<Single<&GlobalTransform, With<Player>>>,
    transforms: Query<&GlobalTransform>,
) {
    let Some(player) = player else { return };
    let player_pos = player.translation();

    for (entity, shooter, npc_transform, aggro_target, alert) in &mut enemies {
        let target_pos = aggro_target
            .and_then(|at| transforms.get(at.0).ok())
            .map(|gt| gt.translation())
            .unwrap_or(player_pos);

        let npc_pos = npc_transform.translation();
        let to_target = target_pos - npc_pos;
        let distance = to_target.length();

        let to_target_hz = Vec3::new(to_target.x, 0.0, to_target.z);
        let forward = npc_transform.forward().as_vec3();
        let forward_hz = Vec3::new(forward.x, 0.0, forward.z);

        let can_see = if distance < 0.01 || distance > shooter.range {
            false
        } else if let (Ok(to_dir), Ok(fwd_dir)) = (Dir3::new(to_target_hz), Dir3::new(forward_hz)) {
            let dot = to_dir.dot(*fwd_dir);
            let in_fov = dot >= DETECTION_HALF_ANGLE.cos(); // cos(60°) = 0.5

            if in_fov {
                // LOS check
                let direction = Dir3::new(to_target).unwrap();
                let los_hit = spatial_query.cast_ray(
                    npc_pos,
                    direction,
                    distance,
                    true,
                    &SpatialQueryFilter::from_mask(CollisionLayer::Level),
                );
                los_hit.is_none()
            } else {
                false
            }
        } else {
            false
        };

        match alert {
            Some(mut alert) if can_see => {
                alert.last_seen_position = target_pos;
                alert.lose_sight_timer.reset();
            }
            Some(mut alert) => {
                // Lost sight — tick the timer
                alert.lose_sight_timer.tick(time.delta());
                if alert.lose_sight_timer.just_finished() {
                    commands.entity(entity).remove::<EnemyAlert>();
                }
            }
            None if can_see => {
                commands.entity(entity).insert(EnemyAlert {
                    last_seen_position: target_pos,
                    lose_sight_timer: Timer::from_seconds(LOSE_SIGHT_DURATION, TimerMode::Once),
                });
            }
            None => {}
        }
    }
}

fn rotate_alert_enemies(
    mut enemies: Query<(&mut Transform, &EnemyAlert), With<EnemyGunner>>,
    time: Res<Time>,
) {
    for (mut transform, alert) in &mut enemies {
        let to_target = alert.last_seen_position - transform.translation;
        let to_target_hz = Vec3::new(to_target.x, 0.0, to_target.z);
        let Ok(target_dir) = Dir3::new(to_target_hz) else {
            continue;
        };
        let target = transform.looking_to(target_dir, Vec3::Y).rotation;
        let decay_rate = f32::ln(600.0);
        transform
            .rotation
            .smooth_nudge(&target, decay_rate, time.delta_secs());
    }
}

fn npc_shoot(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<ProjectileAssets>>,
    mut shooters: Query<
        (
            &mut NpcShooter,
            &GlobalTransform,
            &EnemyAlert,
            Option<&AggroTarget>,
            Option<&Faction>,
        ),
        With<NpcAggro>,
    >,
    player: Option<Single<&GlobalTransform, With<Player>>>,
    transforms: Query<&GlobalTransform>,
) {
    let Some(assets) = assets else { return };
    let Some(player) = player else { return };
    let player_pos = player.translation();

    for (mut shooter, npc_transform, _alert, aggro_target, faction) in &mut shooters {
        let faction = faction
            .cloned()
            .unwrap_or(Faction("enemy".to_string()));
        shooter.fire_rate.tick(time.delta());
        if !shooter.fire_rate.just_finished() {
            continue;
        }

        let npc_pos = npc_transform.translation();

        let target_pos = aggro_target
            .and_then(|at| transforms.get(at.0).ok())
            .map(|gt| gt.translation())
            .unwrap_or(player_pos);
        let to_target = target_pos - npc_pos;

        // Spawn projectiles
        let spawn_pos = npc_pos + Vec3::Y * 0.8; // roughly gun height
        let count = shooter.projectile_count;
        let speed = shooter.projectile_speed;

        match shooter.pattern {
            FiringPattern::RadialBurst => {
                for i in 0..count {
                    let angle = (i as f32 / count as f32) * TAU;
                    let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
                    spawn_projectile(
                        &mut commands,
                        &assets,
                        spawn_pos,
                        dir * speed,
                        faction.clone(),
                    );
                }
            }
            FiringPattern::AimedSpread => {
                let forward_hz = Vec3::new(to_target.x, 0.0, to_target.z).normalize_or_zero();
                if forward_hz.length_squared() < 0.01 {
                    continue;
                }
                for i in 0..count {
                    let t = if count <= 1 {
                        0.0
                    } else {
                        (i as f32 / (count - 1) as f32) * 2.0 - 1.0 // -1..1
                    };
                    let angle = t * SPREAD_HALF_ANGLE;
                    let rot = Quat::from_rotation_y(angle);
                    let dir = rot * forward_hz;
                    spawn_projectile(
                        &mut commands,
                        &assets,
                        spawn_pos,
                        dir * speed,
                        faction.clone(),
                    );
                }
            }
        }

        // Gunshot sound at the enemy's position
        commands.spawn((
            SamplePlayer::new(assets.gunshot.clone()),
            SpatialPool,
            Transform::from_translation(npc_pos),
        ));
    }
}

fn spawn_projectile(
    commands: &mut Commands,
    assets: &ProjectileAssets,
    pos: Vec3,
    velocity: Vec3,
    faction: Faction,
) {
    commands.spawn((
        Name::new("Enemy Projectile"),
        EnemyProjectile,
        faction,
        Projectile {
            velocity,
            lifetime: Timer::from_seconds(PROJECTILE_LIFETIME, TimerMode::Once),
        },
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(assets.material.clone()),
        Transform::from_translation(pos),
        RigidBody::Kinematic,
        Collider::sphere(0.1),
        Sensor,
        CollisionLayers::new(
            CollisionLayer::Projectile,
            [CollisionLayer::Character, CollisionLayer::Level],
        ),
    ));
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut proj) in &mut projectiles {
        transform.translation += proj.velocity * dt;
        proj.lifetime.tick(time.delta());
        if proj.lifetime.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn projectile_hit_player(
    mut commands: Commands,
    spatial_query: SpatialQuery,
    projectiles: Query<(Entity, &GlobalTransform, &Collider, &Faction), With<EnemyProjectile>>,
    mut player: Query<(Entity, &mut PlayerHealth, Option<&Invincible>), With<Player>>,
) {
    let Ok((player_entity, mut health, invincible)) = player.single_mut() else {
        return;
    };

    let player_faction = Faction("player".to_string());

    for (proj_entity, proj_transform, proj_collider, proj_faction) in &projectiles {
        if !proj_faction.can_hurt(&player_faction) {
            continue;
        }

        let hits = spatial_query.shape_intersections(
            proj_collider,
            proj_transform.translation(),
            proj_transform.to_isometry().rotation,
            &SpatialQueryFilter::from_mask(CollisionLayer::Character),
        );

        for hit_entity in &hits {
            if *hit_entity == player_entity {
                hurt_player(&mut commands, player_entity, &mut health, invincible);
                commands.entity(proj_entity).despawn();
                break;
            }
        }
    }
}

fn projectile_hit_npc(
    mut commands: Commands,
    spatial_query: SpatialQuery,
    projectiles: Query<(Entity, &GlobalTransform, &Collider, &Faction), With<EnemyProjectile>>,
    player: Option<Single<Entity, With<Player>>>,
    mut health_query: Query<(&mut Health, Option<&Faction>), Without<Player>>,
) {
    let player_entity = player.map(|p| *p);

    for (proj_entity, proj_transform, proj_collider, proj_faction) in &projectiles {
        if commands.get_entity(proj_entity).is_err() {
            continue;
        }

        let hits = spatial_query.shape_intersections(
            proj_collider,
            proj_transform.translation(),
            proj_transform.to_isometry().rotation,
            &SpatialQueryFilter::from_mask(CollisionLayer::Character),
        );

        for hit_entity in &hits {
            if player_entity == Some(*hit_entity) {
                continue;
            }

            let Ok((mut health, target_faction)) = health_query.get_mut(*hit_entity) else {
                continue;
            };
            let target_faction = target_faction
                .cloned()
                .unwrap_or(Faction("enemy".to_string()));
            if !proj_faction.can_hurt(&target_faction) {
                continue;
            }

            health.0 -= 10.0;
            if health.0 <= 0.0 {
                commands.entity(*hit_entity).insert(NpcDead);
            }
            commands.entity(proj_entity).despawn();
            break;
        }
    }
}

fn projectile_hit_level(
    mut commands: Commands,
    spatial_query: SpatialQuery,
    projectiles: Query<(Entity, &GlobalTransform, &Collider), With<EnemyProjectile>>,
) {
    for (proj_entity, proj_transform, proj_collider) in &projectiles {
        let hits = spatial_query.shape_intersections(
            proj_collider,
            proj_transform.translation(),
            proj_transform.to_isometry().rotation,
            &SpatialQueryFilter::from_mask(CollisionLayer::Level),
        );

        if !hits.is_empty() {
            commands.entity(proj_entity).despawn();
        }
    }
}
