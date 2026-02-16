use bevy::prelude::*;

use super::npc::Health;
use super::player::{PlayerHealth, camera::PlayerCamera};
use crate::{screens::Screen, theme::GameFont};

pub fn plugin(app: &mut App) {
    app.add_observer(spawn_healthbar);
    app.add_systems(OnEnter(Screen::Gameplay), spawn_player_health_bar);
    app.add_systems(
        Update,
        (
            billboard_healthbars,
            update_healthbars,
            update_player_health_bar.run_if(in_state(Screen::Gameplay)),
        ),
    );
}

const BAR_WIDTH: f32 = 1.0;
const BAR_HEIGHT: f32 = 0.08;
const BAR_OFFSET_Y: f32 = 1.8;

/// How long the bar stays fully visible after taking damage.
const SHOW_DURATION: f32 = 2.0;
/// How long the bar takes to fade out after SHOW_DURATION expires.
const FADE_DURATION: f32 = 1.0;

#[derive(Component)]
struct HealthBar {
    target: Entity,
    max_health: f32,
    prev_health: f32,
    show_timer: f32,
    opacity: f32,
}

#[derive(Component)]
struct HealthBarFill;

#[derive(Component)]
struct HealthBarBg;

fn spawn_healthbar(
    add: On<Add, Health>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    health_query: Query<&Health>,
) {
    let entity = add.entity;
    let initial_health = health_query.get(entity).map(|h| h.0).unwrap_or(100.0);

    let bg_mesh = meshes.add(Plane3d::new(
        Vec3::Z,
        Vec2::new(BAR_WIDTH / 2.0, BAR_HEIGHT / 2.0),
    ));
    let fill_mesh = meshes.add(Plane3d::new(
        Vec3::Z,
        Vec2::new(BAR_WIDTH / 2.0, BAR_HEIGHT / 2.0),
    ));

    let bg_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let fill_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.8, 0.1, 0.1, 0.0),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands
        .spawn((
            Name::new("Health Bar"),
            HealthBar {
                target: entity,
                max_health: initial_health,
                prev_health: initial_health,
                show_timer: 0.0,
                opacity: 0.0,
            },
            Transform::from_translation(Vec3::ZERO),
            Visibility::Inherited,
        ))
        .with_children(|parent| {
            // Background
            parent.spawn((
                HealthBarBg,
                Mesh3d(bg_mesh),
                MeshMaterial3d(bg_mat),
                Transform::from_translation(Vec3::new(0.0, 0.0, -0.001)),
            ));

            // Fill
            parent.spawn((
                HealthBarFill,
                Mesh3d(fill_mesh),
                MeshMaterial3d(fill_mat),
                Transform::IDENTITY,
            ));
        });
}

fn billboard_healthbars(
    camera: Option<Single<&GlobalTransform, With<PlayerCamera>>>,
    mut bars: Query<&mut Transform, (With<HealthBar>, Without<PlayerCamera>)>,
) {
    let Some(camera) = camera else { return };
    let cam_pos = camera.translation();

    for mut transform in &mut bars {
        let dir = cam_pos - transform.translation;
        let dir_flat = Vec3::new(dir.x, 0.0, dir.z);
        if dir_flat.length_squared() > 1e-6 {
            transform.look_to(-dir_flat.normalize(), Vec3::Y);
        }
    }
}

fn update_healthbars(
    mut commands: Commands,
    mut bars: Query<(Entity, &mut HealthBar, &Children)>,
    mut fills: Query<
        &mut Transform,
        (
            With<HealthBarFill>,
            Without<HealthBar>,
            Without<HealthBarBg>,
        ),
    >,
    health_query: Query<(&Health, &GlobalTransform)>,
    mut bar_transforms: Query<
        &mut Transform,
        (
            With<HealthBar>,
            Without<HealthBarFill>,
            Without<HealthBarBg>,
        ),
    >,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    fill_mats: Query<
        &MeshMaterial3d<StandardMaterial>,
        (With<HealthBarFill>, Without<HealthBarBg>),
    >,
    bg_mats: Query<&MeshMaterial3d<StandardMaterial>, With<HealthBarBg>>,
) {
    let dt = time.delta_secs();

    for (bar_entity, mut bar, children) in &mut bars {
        let Ok((health, target_transform)) = health_query.get(bar.target) else {
            commands.entity(bar_entity).despawn();
            continue;
        };

        if health.0 < bar.prev_health {
            bar.show_timer = SHOW_DURATION;
            bar.opacity = 1.0;
        }
        bar.prev_health = health.0;

        if bar.show_timer > 0.0 {
            bar.show_timer = (bar.show_timer - dt).max(0.0);
        } else if bar.opacity > 0.0 {
            bar.opacity = (bar.opacity - dt / FADE_DURATION).max(0.0);
        }

        let opacity = bar.opacity;
        for child in children.iter() {
            if let Ok(mat_handle) = fill_mats.get(child) {
                if let Some(mat) = materials.get_mut(&mat_handle.0) {
                    mat.base_color = Color::srgba(0.8, 0.1, 0.1, opacity);
                }
            }
            if let Ok(mat_handle) = bg_mats.get(child) {
                if let Some(mat) = materials.get_mut(&mat_handle.0) {
                    mat.base_color = Color::srgba(0.0, 0.0, 0.0, 0.6 * opacity);
                }
            }
        }

        if let Ok(mut bar_transform) = bar_transforms.get_mut(bar_entity) {
            bar_transform.translation = target_transform.translation() + Vec3::Y * BAR_OFFSET_Y;
        }

        let ratio = (health.0 / bar.max_health).clamp(0.0, 1.0);
        for child in children.iter() {
            if let Ok(mut fill_transform) = fills.get_mut(child) {
                fill_transform.scale.x = ratio;
                fill_transform.translation.x = -(1.0 - ratio) * BAR_WIDTH / 2.0;
            }
        }
    }
}

const PLAYER_BAR_WIDTH: f32 = 200.0;
const PLAYER_BAR_HEIGHT: f32 = 16.0;

#[derive(Component)]
struct PlayerHealthBarFill;

#[derive(Component)]
struct PlayerHealthBarText;

fn spawn_player_health_bar(mut commands: Commands, font: Res<GameFont>) {
    commands
        .spawn((
            Name::new("Player Health Bar"),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                left: Val::Px(24.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(Screen::Gameplay),
        ))
        .with_children(|parent| {
            parent.spawn((
                PlayerHealthBarText,
                Text::new("3 / 3"),
                TextFont {
                    font: font.0.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent
                .spawn((
                    Name::new("Bar Bg"),
                    Node {
                        width: Val::Px(PLAYER_BAR_WIDTH),
                        height: Val::Px(PLAYER_BAR_HEIGHT),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                ))
                .with_children(|bg| {
                    bg.spawn((
                        PlayerHealthBarFill,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.8, 0.15, 0.15)),
                    ));
                });
        });
}

fn update_player_health_bar(
    player: Option<Single<&PlayerHealth>>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<PlayerHealthBarFill>>,
    mut text: Query<&mut Text, With<PlayerHealthBarText>>,
) {
    let Some(health) = player else { return };
    let ratio = health.current as f32 / health.max.max(1) as f32;

    for (mut node, mut bg) in &mut fill {
        node.width = Val::Percent(ratio * 100.0);
        let color = if ratio > 0.5 {
            Color::srgb(0.2, 0.7, 0.2)
        } else if ratio > 0.25 {
            Color::srgb(0.8, 0.6, 0.1)
        } else {
            Color::srgb(0.8, 0.15, 0.15)
        };
        *bg = BackgroundColor(color);
    }

    for mut t in &mut text {
        **t = format!("{} / {}", health.current, health.max);
    }
}
