use std::iter;

use avian3d::prelude::*;
use bevy::{
    camera::visibility::RenderLayers, input::common_conditions::input_pressed,
    light::NotShadowCaster, prelude::*, scene::SceneInstanceReady,
};
use bevy_ahoy::CharacterController;
use bevy_enhanced_input::prelude::*;
use bevy_hanabi::ParticleEffect;

use crate::{
    RenderLayer,
    asset_tracking::LoadResource,
    gameplay::{
        dig::{VOXEL_SIZE, Voxel, VoxelSim},
        effects::{DigParticleEffect, MuzzleFlashEffect, ParticleEffectOf, ParticleEffects},
        npc::Health,
        player::camera::PlayerCamera,
    },
    screens::Screen,
    third_party::avian3d::CollisionLayer,
};

pub fn plugin(app: &mut App) {
    app.init_resource::<Inventory>();
    app.load_resource::<InventoryAssets>();
    app.add_systems(OnEnter(Screen::Gameplay), spawn_inventory_hud);
    app.add_systems(
        Update,
        update_inventory_hud.run_if(resource_changed::<Inventory>),
    );
    app.add_systems(
        Update,
        update_held_item.run_if(resource_changed::<Inventory>.or(held_item_missing)),
    );

    app.add_systems(
        Update,
        (
            (
                tick_item_cooldowns,
                (use_shovel, use_tommygun).run_if(input_pressed(MouseButton::Left)),
            )
                .chain(),
            animate_shovel_swing,
            animate_gun_recoil,
        ),
    );
    app.add_observer(on_select_slot::<SelectSlot1, 0>);
    app.add_observer(on_select_slot::<SelectSlot2, 1>);
    app.add_observer(on_select_slot::<SelectSlot3, 2>);
}

#[derive(Resource)]
pub(crate) struct Inventory {
    pub slots: [Option<Item>; 3],
    pub active_slot: usize,
    pub using_hands: bool,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: [Some(Item::Shovel), Some(Item::Gun), None],
            active_slot: 0,
            using_hands: false,
        }
    }
}

impl Inventory {
    pub fn active_item(&self) -> Option<&Item> {
        if self.using_hands {
            None
        } else {
            self.slots[self.active_slot].as_ref()
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Item {
    Shovel,
    Gun,
}

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct SelectSlot1;

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct SelectSlot2;

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct SelectSlot3;

fn on_select_slot<Action: InputAction, const N: usize>(
    _on: On<Start<Action>>,
    mut inventory: ResMut<Inventory>,
) {
    if inventory.active_slot == N && !inventory.using_hands {
        inventory.using_hands = true;
    } else {
        inventory.active_slot = N;
        inventory.using_hands = false;
    }
}

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct UseTool;

const DIG_DISTANCE: f32 = 5.0;
pub const DIG_RADIUS: f32 = 2.0;
const DIG_COOLDOWN: f32 = 0.5;

const GUN_DISTANCE: f32 = 50.0;
const GUN_DAMAGE: f32 = 10.0;
const GUN_COOLDOWN: f32 = 0.1;
const GUN_RECOIL_DURATION: f32 = GUN_COOLDOWN * 0.5;
const GUN_RECOIL_Z: f32 = 0.3;
const GUN_RETURN_SPEED: f32 = 20.0;
const GUN_REST_TRANSLATION: Vec3 = Vec3::new(1.5, -0.3, -2.0);

#[derive(Component)]
struct ItemCooldown {
    timer: Timer,
    ready: bool,
}
impl ItemCooldown {
    pub fn shovel() -> Self {
        Self {
            timer: Timer::from_seconds(DIG_COOLDOWN, TimerMode::Once),
            ready: true,
        }
    }
    pub fn gun() -> Self {
        Self {
            timer: Timer::from_seconds(GUN_COOLDOWN, TimerMode::Once),
            ready: true,
        }
    }
}

fn tick_item_cooldowns(time: Res<Time>, cooldowns: Query<&mut ItemCooldown>) {
    for mut cooldown in cooldowns {
        cooldown.timer.tick(time.delta());
        if cooldown.timer.just_finished() {
            cooldown.ready = true;
        }
    }
}

fn use_shovel(
    player: Single<&GlobalTransform, With<PlayerCamera>>,
    spatial_query: SpatialQuery,
    mut voxel_sims: Query<(&mut VoxelSim, &GlobalTransform)>,
    shovel: Single<(&mut ItemCooldown, &mut ItemAnimation, &ParticleEffects)>,
    mut effects: Query<&mut Transform, With<ParticleEffectOf>>,
) {
    let (mut cooldown, mut swing, effect) = shovel.into_inner();
    if !cooldown.ready {
        return;
    }
    if let Ok(mut transform) = effects.get_mut(effect.entity())
        && let Some(hit_point) = dig_voxel(&player, &spatial_query, &mut voxel_sims)
    {
        *transform = Transform::from_translation(hit_point);
    }
    cooldown.timer.reset();
    cooldown.ready = false;
    swing.timer.reset();
    swing.returning = false;
}

// run if mouse button left pressed
fn use_tommygun(
    mut commands: Commands,
    player: Single<&GlobalTransform, With<PlayerCamera>>,
    mut health_query: Query<&mut Health>,
    spatial_query: SpatialQuery,
    gun: Single<(&mut ItemCooldown, &mut ItemAnimation)>,
) {
    let (mut cooldown, mut recoil) = gun.into_inner();

    if !cooldown.ready {
        return;
    }

    let camera_transform = player.compute_transform();
    let origin = camera_transform.translation;
    let direction = camera_transform.forward();

    if let Some(hit) = spatial_query.cast_ray(
        origin,
        direction,
        GUN_DISTANCE,
        true,
        &SpatialQueryFilter::from_mask([CollisionLayer::Default, CollisionLayer::Character]),
    ) {
        if let Ok(mut health) = health_query.get_mut(hit.entity) {
            health.0 -= GUN_DAMAGE;
            if health.0 <= 0.0 {
                commands.entity(hit.entity).despawn();
            }
        }
    }

    cooldown.timer.reset();
    cooldown.ready = false;
    recoil.timer.reset();
    recoil.returning = false;
}

/// Returns the world-space hit point if voxels were dug.
fn dig_voxel(
    player: &GlobalTransform,
    spatial_query: &SpatialQuery,
    voxel_sims: &mut Query<(&mut VoxelSim, &GlobalTransform)>,
) -> Option<Vec3> {
    let camera_transform = player.compute_transform();
    let origin = camera_transform.translation;
    let direction = camera_transform.forward();

    let hit = spatial_query.cast_ray(
        origin,
        direction,
        DIG_DISTANCE,
        true,
        &SpatialQueryFilter::from_mask(CollisionLayer::Default),
    )?;

    let Ok((mut sim, sim_transform)) = voxel_sims.get_mut(hit.entity) else {
        return None;
    };

    // push it in a little bit so we aren't at the edge of a voxel
    const BIAS: f32 = 0.1;
    let hit_point = origin + *direction * hit.distance + *direction * BIAS;
    let surface_point = origin + *direction * hit.distance;

    let local = sim_transform
        .compute_transform()
        .compute_affine()
        .inverse()
        .transform_point3(hit_point);
    let center = (local / VOXEL_SIZE).floor().as_ivec3();

    let r = DIG_RADIUS as i32;
    let r_sq = DIG_RADIUS * DIG_RADIUS;
    for dx in -r..=r {
        for dy in -r..=r {
            for dz in -r..=r {
                let dist_sq = (dx * dx + dy * dy + dz * dz) as f32;
                if dist_sq <= r_sq {
                    let pos = center + IVec3::new(dx, dy, dz);
                    sim.set(pos, Voxel::Air);
                }
            }
        }
    }

    Some(surface_point)
}

const SLOT_SIZE: f32 = 60.0;
const SLOT_GAP: f32 = 8.0;
const ACTIVE_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.4);
const INACTIVE_COLOR: Color = Color::srgba(0.3, 0.3, 0.3, 0.4);

#[derive(Component)]
struct InventorySlotUi(usize);

fn spawn_inventory_hud(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Inventory HUD"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::End,
                padding: UiRect::bottom(Val::Px(20.0)),
                ..default()
            },
            DespawnOnExit(Screen::Gameplay),
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    column_gap: Val::Px(SLOT_GAP),
                    ..default()
                })
                .with_children(|row| {
                    for i in 0..3 {
                        let bg = if i == 0 { ACTIVE_COLOR } else { INACTIVE_COLOR };
                        row.spawn((
                            Name::new(format!("Slot {}", i + 1)),
                            InventorySlotUi(i),
                            Node {
                                width: Val::Px(SLOT_SIZE),
                                height: Val::Px(SLOT_SIZE),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(bg),
                            BorderColor::all(Color::WHITE),
                        ))
                        .with_child((
                            Text::new(""),
                            TextFont {
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    }
                });
        });
}

fn update_inventory_hud(
    inventory: Res<Inventory>,
    mut slots: Query<(&InventorySlotUi, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut Text>,
) {
    for (slot_ui, mut bg, children) in &mut slots {
        let index = slot_ui.0;
        let is_active = index == inventory.active_slot;
        *bg = if is_active {
            ACTIVE_COLOR
        } else {
            INACTIVE_COLOR
        }
        .into();

        // kinda wanna display a rotating tool for each of these, would be funny
        let item_name = if is_active && inventory.using_hands {
            "Hands".to_string()
        } else {
            inventory.slots[index]
                .as_ref()
                .map(|item| format!("{:?}", item))
                .unwrap_or(String::new())
        };

        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = item_name.to_string();
            }
        }
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
struct InventoryAssets {
    #[dependency]
    shovel: Handle<Scene>,
    #[dependency]
    gun: Handle<Scene>,
}

impl FromWorld for InventoryAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            shovel: assets.load("models/shovel/scene.gltf#Scene0"),
            gun: assets.load("models/tommy_gun.glb#Scene0"),
        }
    }
}

#[derive(Component)]
struct HeldItemModel;

fn held_item_missing(inventory: Res<Inventory>, existing: Query<(), With<HeldItemModel>>) -> bool {
    inventory.active_item().is_some() && existing.is_empty()
}

const SHOVEL_SWING_X_END: f32 = 0.0;
const SHOVEL_SWING_X_START: f32 = -1.7;
const SHOVEL_REST_ROTATION: Vec3 = Vec3::new(SHOVEL_SWING_X_START, 3.00, -1.7);
const SHOVEL_SWING_DURATION: f32 = DIG_COOLDOWN * 0.7;
const SHOVEL_RETURN_SPEED: f32 = 12.0;

#[derive(Component)]
#[require(ItemCooldown = ItemCooldown::shovel())]
struct ItemAnimation {
    timer: Timer,
    returning: bool,
    current_offset: f32,
}

impl ItemAnimation {
    pub fn gun() -> Self {
        let mut timer = Timer::from_seconds(GUN_RECOIL_DURATION, TimerMode::Once);
        timer.tick(timer.duration());
        Self {
            timer,
            returning: true,
            current_offset: GUN_REST_TRANSLATION.z,
        }
    }

    pub fn shovel() -> Self {
        let mut timer = Timer::from_seconds(SHOVEL_SWING_DURATION, TimerMode::Once);
        timer.tick(timer.duration());
        Self {
            timer,
            returning: true,
            current_offset: SHOVEL_SWING_X_START,
        }
    }
}
#[derive(Component)]
pub struct Shovel;

#[derive(Component)]
pub struct Gun;

fn update_held_item(
    mut commands: Commands,
    inventory: Res<Inventory>,
    existing: Query<Entity, With<HeldItemModel>>,
    player_camera: Single<Entity, With<PlayerCamera>>,
    inventory_assets: Res<InventoryAssets>,
    muzzle_effect: Res<MuzzleFlashEffect>,
    dig_effect: Res<DigParticleEffect>,
    // mut last_held: Local<Option<Item>>,
) {
    let camera_entity = *player_camera;

    for entity in &existing {
        commands.entity(entity).despawn();
    }

    match inventory.active_item() {
        Some(Item::Shovel) => {
            let held = commands
                .spawn((
                    Name::new("Held Shovel"),
                    HeldItemModel,
                    ItemAnimation::shovel(),
                    ItemCooldown::shovel(),
                    Shovel,
                    SceneRoot(inventory_assets.shovel.clone()),
                    Transform {
                        translation: Vec3::new(0.4, -0.2, -0.5),
                        rotation: Quat::from_euler(
                            EulerRot::XYZ,
                            SHOVEL_REST_ROTATION.x,
                            SHOVEL_REST_ROTATION.y,
                            SHOVEL_REST_ROTATION.z,
                        ),
                        ..default()
                    },
                ))
                .observe(configure_held_item_view_model)
                .id();

            commands.spawn((
                ParticleEffect::new(dig_effect.0.clone()),
                RenderLayers::from(RenderLayer::DEFAULT),
                Transform::default(),
                ParticleEffectOf(held),
            ));

            commands.entity(camera_entity).add_child(held);
        }
        Some(Item::Gun) => {
            let held = commands
                .spawn((
                    Name::new("Held Gun"),
                    HeldItemModel,
                    ItemAnimation::gun(),
                    ItemCooldown::gun(),
                    Gun,
                    SceneRoot(inventory_assets.gun.clone()),
                    Transform {
                        translation: GUN_REST_TRANSLATION,
                        rotation: Quat::from_euler(EulerRot::XYZ, 0.0, -1.58, -0.035),
                        scale: Vec3::splat(0.01),
                    },
                ))
                .observe(configure_held_item_view_model)
                .id();

            commands.spawn((
                ParticleEffect::new(muzzle_effect.0.clone()),
                RenderLayers::from(RenderLayer::DEFAULT),
                Transform::from_translation(Vec3::new(-20., 0., 0.)),
                ParticleEffectOf(held),
                ChildOf(held),
            ));

            commands.entity(camera_entity).add_child(held);
        }
        None => {}
    }
}

// i love hardcoding animations c:
fn animate_shovel_swing(
    time: Res<Time>,
    mut query: Query<(&mut ItemAnimation, &mut Transform), With<Shovel>>,
) {
    for (mut swing, mut transform) in &mut query {
        swing.timer.tick(time.delta());

        let x = if swing.returning {
            let target = SHOVEL_SWING_X_START;
            swing.current_offset +=
                (target - swing.current_offset) * SHOVEL_RETURN_SPEED * time.delta_secs();
            if (swing.current_offset - target).abs() < 0.01 {
                swing.current_offset = target;
            }
            swing.current_offset
        } else if swing.timer.just_finished()
            || swing.timer.elapsed_secs() >= swing.timer.duration().as_secs_f32()
        {
            swing.returning = true;
            swing.current_offset = SHOVEL_SWING_X_END;
            SHOVEL_SWING_X_END
        } else {
            let t =
                (swing.timer.elapsed_secs() / swing.timer.duration().as_secs_f32()).clamp(0.0, 1.0);
            let x = SHOVEL_SWING_X_START + (SHOVEL_SWING_X_END - SHOVEL_SWING_X_START) * t;
            swing.current_offset = x;
            x
        };

        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            x,
            SHOVEL_REST_ROTATION.y,
            SHOVEL_REST_ROTATION.z,
        );
    }
}

fn animate_gun_recoil(
    time: Res<Time>,
    mut query: Query<(&mut ItemAnimation, &mut Transform), With<Gun>>,
) {
    for (mut recoil, mut transform) in &mut query {
        recoil.timer.tick(time.delta());

        let z = if recoil.returning {
            let target = GUN_REST_TRANSLATION.z;
            recoil.current_offset +=
                (target - recoil.current_offset) * GUN_RETURN_SPEED * time.delta_secs();
            if (recoil.current_offset - target).abs() < 0.001 {
                recoil.current_offset = target;
            }
            recoil.current_offset
        } else if recoil.timer.just_finished()
            || recoil.timer.elapsed_secs() >= recoil.timer.duration().as_secs_f32()
        {
            recoil.returning = true;
            let kicked = GUN_REST_TRANSLATION.z + GUN_RECOIL_Z;
            recoil.current_offset = kicked;
            kicked
        } else {
            let t = (recoil.timer.elapsed_secs() / recoil.timer.duration().as_secs_f32())
                .clamp(0.0, 1.0);
            let z = GUN_REST_TRANSLATION.z + (GUN_RECOIL_Z) * t;
            recoil.current_offset = z;
            z
        };

        transform.translation.z = z;
    }
}

fn configure_held_item_view_model(
    ready: On<SceneInstanceReady>,
    mut commands: Commands,
    q_children: Query<&Children>,
    q_mesh: Query<(), With<Mesh3d>>,
) {
    let root = ready.entity;

    for child in iter::once(root)
        .chain(q_children.iter_descendants(root))
        .filter(|e| q_mesh.contains(*e))
    {
        commands
            .entity(child)
            .insert((RenderLayers::from(RenderLayer::VIEW_MODEL), NotShadowCaster));
    }
}
