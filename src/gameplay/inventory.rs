use std::iter;

use avian3d::prelude::*;
use bevy::{
    camera::visibility::RenderLayers, light::NotShadowCaster, prelude::*, scene::SceneInstanceReady,
};
use bevy_enhanced_input::prelude::*;

use crate::{
    RenderLayer,
    gameplay::{
        dig::{VOXEL_SIZE, VoxelSim},
        player::camera::PlayerCamera,
    },
    screens::Screen,
    third_party::avian3d::CollisionLayer,
};

pub fn plugin(app: &mut App) {
    app.init_resource::<Inventory>();
    app.add_systems(OnEnter(Screen::Gameplay), spawn_inventory_hud);
    app.add_systems(
        Update,
        (update_inventory_hud, update_held_item).run_if(resource_changed::<Inventory>),
    );
    app.add_observer(on_select_slot::<SelectSlot1, 0>);
    app.add_observer(on_select_slot::<SelectSlot2, 1>);
    app.add_observer(on_select_slot::<SelectSlot3, 2>);
    app.add_observer(on_use_tool);
}

#[derive(Resource)]
pub(crate) struct Inventory {
    pub slots: [Option<Item>; 3],
    pub active_slot: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: [Some(Item::Shovel), Some(Item::Gun), None],
            active_slot: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
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
    inventory.active_slot = N;
}

#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct UseTool;

const DIG_DISTANCE: f32 = 5.0;
const DIG_RADIUS: f32 = 3.0;

fn on_use_tool(
    _on: On<Start<UseTool>>,
    inventory: Res<Inventory>,
    player: Single<&GlobalTransform, With<PlayerCamera>>,
    spatial_query: SpatialQuery,
    mut voxel_sims: Query<(&mut VoxelSim, &GlobalTransform)>,
) {
    let active_item = &inventory.slots[inventory.active_slot];
    match active_item {
        Some(Item::Shovel) => dig_voxel(&player, &spatial_query, &mut voxel_sims),
        _ => {}
    }
}

fn dig_voxel(
    player: &GlobalTransform,
    spatial_query: &SpatialQuery,
    voxel_sims: &mut Query<(&mut VoxelSim, &GlobalTransform)>,
) {
    let camera_transform = player.compute_transform();
    let origin = camera_transform.translation;
    let direction = camera_transform.forward();

    let Some(hit) = spatial_query.cast_ray(
        origin,
        direction,
        DIG_DISTANCE,
        true,
        &SpatialQueryFilter::from_mask(CollisionLayer::Default),
    ) else {
        return;
    };

    let Ok((mut sim, sim_transform)) = voxel_sims.get_mut(hit.entity) else {
        return;
    };

    // push it in a little bit so we aren't at the edge of a voxel
    const BIAS: f32 = 0.1;
    let hit_point = origin + *direction * hit.distance + *direction * BIAS;

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
                    sim.set(pos, crate::gameplay::dig::Voxel::Air);
                }
            }
        }
    }
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
        let idx = slot_ui.0;
        *bg = if idx == inventory.active_slot {
            ACTIVE_COLOR
        } else {
            INACTIVE_COLOR
        }
        .into();

        // kinda wanna display a rotating tool for each of these, would be funny
        let item_name = inventory.slots[idx]
            .as_ref()
            .map(|item| format!("{:?}", item))
            .unwrap_or(String::new());

        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = item_name.to_string();
            }
        }
    }
}

#[derive(Component)]
struct HeldItemModel;

fn update_held_item(
    mut commands: Commands,
    inventory: Res<Inventory>,
    existing: Query<Entity, With<HeldItemModel>>,
    player_camera: Single<Entity, With<PlayerCamera>>,
    asset_server: Res<AssetServer>,
) {
    // Despawn any existing held item
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let active_item = &inventory.slots[inventory.active_slot];
    let camera_entity = *player_camera;

    match active_item {
        Some(Item::Shovel) => {
            let held = commands
                .spawn((
                    Name::new("Held Shovel"),
                    HeldItemModel,
                    SceneRoot(asset_server.load("models/shovel/scene.gltf#Scene0")),
                    Transform {
                        translation: Vec3::new(0.4, -0.2, -0.5),
                        rotation: Quat::from_euler(EulerRot::XYZ, 0.0, 3.0, -1.7),
                        ..default()
                    },
                ))
                .observe(configure_held_item_view_model)
                .id();
            commands.entity(camera_entity).add_child(held);
        }
        Some(Item::Gun) => {
            let held = commands
                .spawn((
                    Name::new("Held Gun"),
                    HeldItemModel,
                    SceneRoot(asset_server.load("models/tommy_gun.glb#Scene0")),
                    Transform::from_xyz(0.3, -0.2, -0.5),
                ))
                .observe(configure_held_item_view_model)
                .id();
            commands.entity(camera_entity).add_child(held);
        }
        None => {}
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
