//! Store for buying upgrades to shovel/bucket/gun

use std::any::Any as _;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_mod_billboard::prelude::*;
use bevy_trenchbroom::prelude::*;

use crate::{
    PostPhysicsAppSystems,
    gameplay::{
        crosshair::CrosshairState,
        crusts::Crusts,
        inventory::{Inventory, Item},
        player::{Player, PlayerHealth, camera::PlayerCamera, input::Interact},
    },
    screens::Screen,
    theme::GameFont,
    third_party::avian3d::CollisionLayer,
};

const UPGRADE_INTERACT_DISTANCE: f32 = 3.0;
const CUBE_SIZE: f32 = 0.5;
const TEXT_SCALE: Vec3 = Vec3::splat(0.01);

pub fn plugin(app: &mut App) {
    app.add_plugins(BillboardPlugin);
    app.init_resource::<LookedAtUpgrade>();
    app.init_resource::<UpgradeLevels>();
    app.add_observer(on_add_upgrade_station);
    app.add_observer(interact_with_upgrade);
    app.add_systems(
        Update,
        (
            check_looking_at_upgrade
                .run_if(in_state(Screen::Gameplay))
                .in_set(PostPhysicsAppSystems::ChangeUi),
            update_upgrade_text.run_if(resource_changed::<UpgradeLevels>),
        ),
    );
}

#[derive(Resource, Default)]
pub(crate) struct UpgradeLevels {
    pub shovel_radius: u32,
    pub shovel_speed: u32,
    pub bucket_radius: u32,
    pub bucket_speed: u32,
    pub gun_damage: u32,
    pub gun_firerate: u32,
    pub max_hp: u32,
}

impl UpgradeLevels {
    fn level_for(&self, upgrade: &str) -> u32 {
        match upgrade {
            "shovel_radius" => self.shovel_radius,
            "shovel_speed" => self.shovel_speed,
            "bucket_radius" => self.bucket_radius,
            "bucket_speed" => self.bucket_speed,
            "gun_damage" => self.gun_damage,
            "gun_firerate" => self.gun_firerate,
            "max_hp" => self.max_hp,
            _ => 0,
        }
    }

    fn increment(&mut self, upgrade: &str) {
        match upgrade {
            "shovel_radius" => self.shovel_radius += 1,
            "shovel_speed" => self.shovel_speed += 1,
            "bucket_radius" => self.bucket_radius += 1,
            "bucket_speed" => self.bucket_speed += 1,
            "gun_damage" => self.gun_damage += 1,
            "gun_firerate" => self.gun_firerate += 1,
            "max_hp" => self.max_hp += 1,
            _ => {}
        }
    }

    fn cost_for(&self, upgrade: &str) -> u32 {
        1u32.checked_shl(self.level_for(upgrade))
            .unwrap_or(u32::MAX)
    }
}

fn display_name(upgrade: &str) -> &str {
    match upgrade {
        "shovel_radius" => "Shovel Radius",
        "shovel_speed" => "Shovel Speed",
        "bucket_radius" => "Bucket Radius",
        "bucket_speed" => "Bucket Speed",
        "gun_damage" => "Gun Damage",
        "gun_firerate" => "Gun Firerate",
        "max_hp" => "Max HP",
        _ => "Unknown",
    }
}

fn upgrade_label(upgrade: &str, cost: u32) -> String {
    let name = display_name(upgrade);
    let plural = if cost == 1 { "" } else { "s" };
    format!("{name}\n{cost} crust{plural}")
}

#[point_class(base(Transform, Visibility))]
pub(crate) struct UpgradeStation {
    pub upgrade: String,
}

impl Default for UpgradeStation {
    fn default() -> Self {
        Self {
            upgrade: String::new(),
        }
    }
}

#[derive(Component)]
struct UpgradeText {
    upgrade: String,
}

#[derive(Resource, Default)]
struct LookedAtUpgrade(Option<Entity>);

fn on_add_upgrade_station(
    add: On<Add, UpgradeStation>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stations: Query<&UpgradeStation>,
    upgrade_levels: Res<UpgradeLevels>,
    font: Res<GameFont>,
) {
    let entity = add.entity;
    let Ok(station) = stations.get(entity) else {
        return;
    };

    let cost = upgrade_levels.cost_for(&station.upgrade);
    let label = upgrade_label(&station.upgrade, cost);

    let cube_mesh = meshes.add(Cuboid::new(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.6, 0.3),
        ..default()
    });

    commands.entity(entity).insert((
        Collider::cuboid(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE),
        RigidBody::Static,
        CollisionLayers::new(CollisionLayer::Prop, LayerMask::ALL),
    ));

    commands.entity(entity).with_children(|parent| {
        parent.spawn((Mesh3d(cube_mesh), MeshMaterial3d(material)));
        parent.spawn((
            UpgradeText {
                upgrade: station.upgrade.clone(),
            },
            BillboardText::new(label),
            TextFont {
                font: font.0.clone(),
                font_size: 36.0,
                ..default()
            },
            TextColor(Color::WHITE),
            TextLayout::new_with_justify(Justify::Center),
            Transform::from_translation(Vec3::new(0.0, CUBE_SIZE + 0.3, 0.0))
                .with_scale(TEXT_SCALE),
        ));
    });
}

fn check_looking_at_upgrade(
    player: Single<&GlobalTransform, With<PlayerCamera>>,
    spatial_query: SpatialQuery,
    stations: Query<(), With<UpgradeStation>>,
    mut crosshair: Single<&mut CrosshairState>,
    mut looked_at: ResMut<LookedAtUpgrade>,
) {
    let camera_transform = player.compute_transform();
    let system_id = check_looking_at_upgrade.type_id();

    if let Some(hit) = spatial_query.cast_ray(
        camera_transform.translation,
        camera_transform.forward(),
        UPGRADE_INTERACT_DISTANCE,
        true,
        &SpatialQueryFilter::from_mask(CollisionLayer::Prop),
    ) {
        if stations.get(hit.entity).is_ok() {
            looked_at.0 = Some(hit.entity);
            crosshair.wants_square.insert(system_id);
            return;
        }
    }

    looked_at.0 = None;
    crosshair.wants_square.remove(&system_id);
}

fn interact_with_upgrade(
    _on: On<Start<Interact>>,
    looked_at: Res<LookedAtUpgrade>,
    stations: Query<&UpgradeStation>,
    mut crusts: ResMut<Crusts>,
    mut inventory: ResMut<Inventory>,
    mut upgrade_levels: ResMut<UpgradeLevels>,
    mut player_health: Single<&mut PlayerHealth, With<Player>>,
) {
    let Some(entity) = looked_at.0 else {
        return;
    };
    let Ok(station) = stations.get(entity) else {
        return;
    };

    let cost = upgrade_levels.cost_for(&station.upgrade);
    // if !crusts.try_spend(cost) {
    //     return;
    // }

    apply_upgrade(&station.upgrade, &mut inventory, &mut player_health);
    upgrade_levels.increment(&station.upgrade);
    info!(
        "Upgraded {}! Level {} -> {}",
        display_name(&station.upgrade),
        upgrade_levels.level_for(&station.upgrade) - 1,
        upgrade_levels.level_for(&station.upgrade),
    );
}

fn apply_upgrade(upgrade: &str, inventory: &mut Inventory, player_health: &mut PlayerHealth) {
    match upgrade {
        "shovel_radius" => {
            if let Some(Item::Shovel(stats)) = &mut inventory.slots[0] {
                stats.radius += 0.5;
            }
        }
        "shovel_speed" => {
            if let Some(Item::Shovel(stats)) = &mut inventory.slots[0] {
                stats.cooldown = (stats.cooldown - 0.05).max(0.05);
            }
        }
        "bucket_radius" => {
            if let Some(Item::DirtBucket(stats)) = &mut inventory.slots[2] {
                stats.radius += 0.5;
            }
        }
        "bucket_speed" => {
            if let Some(Item::DirtBucket(stats)) = &mut inventory.slots[2] {
                stats.cooldown = (stats.cooldown - 0.05).max(0.05);
            }
        }
        "gun_damage" => {
            if let Some(Item::Gun(stats)) = &mut inventory.slots[1] {
                stats.damage += 3.0;
            }
        }
        "gun_firerate" => {
            if let Some(Item::Gun(stats)) = &mut inventory.slots[1] {
                stats.cooldown = (stats.cooldown - 0.01).max(0.01);
            }
        }
        "max_hp" => {
            player_health.max += 1;
            player_health.current = player_health
                .current
                .saturating_add(1)
                .min(player_health.max);
        }
        _ => {
            warn!("Unknown upgrade type: {upgrade}");
        }
    }
}

fn update_upgrade_text(
    upgrade_levels: Res<UpgradeLevels>,
    mut texts: Query<(&UpgradeText, &mut BillboardText)>,
) {
    for (upgrade_text, mut text) in &mut texts {
        let cost = upgrade_levels.cost_for(&upgrade_text.upgrade);
        text.0 = upgrade_label(&upgrade_text.upgrade, cost);
    }
}
