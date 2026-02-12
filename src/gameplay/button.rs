use std::any::Any as _;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_trenchbroom::prelude::*;

use crate::{
    PostPhysicsAppSystems,
    gameplay::{crosshair::CrosshairState, player::camera::PlayerCamera},
    screens::Screen,
    third_party::avian3d::CollisionLayer,
};

const BUTTON_INTERACT_DISTANCE: f32 = 3.0;

pub fn plugin(app: &mut App) {
    app.add_observer(on_add_button);
    app.add_systems(
        Update,
        check_looking_at_button
            .run_if(in_state(Screen::Gameplay))
            .in_set(PostPhysicsAppSystems::ChangeUi),
    );
}

fn on_add_button(
    add: On<Add, Button>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.entity(add.entity).insert((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.3, 0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.1, 0.1),
            ..default()
        })),
        Collider::cuboid(0.5, 0.3, 0.5),
        RigidBody::Static,
        CollisionLayers::new(CollisionLayer::Prop, LayerMask::ALL),
    ));
}

#[point_class(base(Transform, Visibility))]
pub(crate) struct Button {
    pub trigger: String,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            trigger: String::new(),
        }
    }
}

fn check_looking_at_button(
    player: Single<&GlobalTransform, With<PlayerCamera>>,
    spatial_query: SpatialQuery,
    buttons: Query<(), With<Button>>,
    mut crosshair: Single<&mut CrosshairState>,
) {
    let camera_transform = player.compute_transform();
    let system_id = check_looking_at_button.type_id();

    if let Some(hit) = spatial_query.cast_ray(
        camera_transform.translation,
        camera_transform.forward(),
        BUTTON_INTERACT_DISTANCE,
        true,
        &SpatialQueryFilter::from_mask(CollisionLayer::Prop),
    ) {
        if buttons.get(hit.entity).is_ok() {
            crosshair.wants_square.insert(system_id);
            return;
        }
    }

    crosshair.wants_square.remove(&system_id);
}
