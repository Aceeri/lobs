use std::any::Any as _;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use bevy_trenchbroom::prelude::*;

use crate::{
    PostPhysicsAppSystems,
    gameplay::{
        crosshair::CrosshairState,
        player::{camera::PlayerCamera, input::Interact},
    },
    screens::Screen,
    third_party::avian3d::CollisionLayer,
};

const BUTTON_INTERACT_DISTANCE: f32 = 3.0;
const BUTTON_TOP_HEIGHT: f32 = 0.12;
const BUTTON_TOP_WIDTH: f32 = 0.35;
const BUTTON_BASE_HEIGHT: f32 = 0.15;
const BUTTON_BASE_WIDTH: f32 = 0.5;
const BUTTON_TOP_EMBED: f32 = 0.04;
const BUTTON_PRESS_DURATION: f32 = 0.15;
const BUTTON_RETURN_SPEED: f32 = 4.0;
const BUTTON_PRESSED_SCALE: f32 = 0.3;

pub fn plugin(app: &mut App) {
    app.init_resource::<LookedAtButton>();
    app.add_observer(on_add_button);
    app.add_observer(interact_with_button);
    app.add_systems(
        Update,
        (
            check_looking_at_button
                .run_if(in_state(Screen::Gameplay))
                .in_set(PostPhysicsAppSystems::ChangeUi),
            animate_button_press,
        ),
    );
}

#[derive(Component)]
struct ButtonTop;

#[derive(Component)]
struct ButtonPress {
    timer: Timer,
    returning: bool,
    current_scale: f32,
}

impl Default for ButtonPress {
    fn default() -> Self {
        let mut timer = Timer::from_seconds(BUTTON_PRESS_DURATION, TimerMode::Once);
        timer.tick(timer.duration());
        Self {
            timer,
            returning: true,
            current_scale: 1.0,
        }
    }
}

fn on_add_button(
    add: On<Add, Button>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let top_mesh = meshes.add(Cuboid::new(BUTTON_TOP_WIDTH, BUTTON_TOP_HEIGHT, BUTTON_TOP_WIDTH));
    let base_mesh = meshes.add(Cuboid::new(BUTTON_BASE_WIDTH, BUTTON_BASE_HEIGHT, BUTTON_BASE_WIDTH));

    let red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.1, 0.1),
        ..default()
    });
    let grey = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.2, 0.2),
        ..default()
    });

    let total_height = BUTTON_TOP_HEIGHT + BUTTON_BASE_HEIGHT - BUTTON_TOP_EMBED;

    commands.entity(add.entity).insert((
        Collider::cuboid(BUTTON_BASE_WIDTH, total_height, BUTTON_BASE_WIDTH),
        RigidBody::Static,
        CollisionLayers::new(CollisionLayer::Prop, LayerMask::ALL),
    ));

    let base_y = -BUTTON_TOP_HEIGHT / 2.0 + BUTTON_TOP_EMBED / 2.0;
    let top_y = BUTTON_BASE_HEIGHT / 2.0 - BUTTON_TOP_EMBED;

    commands.entity(add.entity).with_children(|parent| {
        parent.spawn((
            Name::new("Button Base"),
            Mesh3d(base_mesh),
            MeshMaterial3d(grey),
            Transform::from_translation(Vec3::new(0.0, base_y, 0.0)),
        ));
        parent.spawn((
            Name::new("Button Top"),
            ButtonTop,
            ButtonPress::default(),
            Mesh3d(top_mesh),
            MeshMaterial3d(red),
            Transform::from_translation(Vec3::new(0.0, top_y, 0.0)),
        ));
    });
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

#[derive(Resource, Default)]
struct LookedAtButton(Option<Entity>);

fn check_looking_at_button(
    player: Single<&GlobalTransform, With<PlayerCamera>>,
    spatial_query: SpatialQuery,
    buttons: Query<(), With<Button>>,
    mut crosshair: Single<&mut CrosshairState>,
    mut looked_at: ResMut<LookedAtButton>,
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
            looked_at.0 = Some(hit.entity);
            crosshair.wants_square.insert(system_id);
            return;
        }
    }

    looked_at.0 = None;
    crosshair.wants_square.remove(&system_id);
}

fn interact_with_button(
    _on: On<Start<Interact>>,
    looked_at: Res<LookedAtButton>,
    buttons: Query<&Button>,
    children: Query<&Children>,
    mut presses: Query<&mut ButtonPress>,
) {
    let Some(entity) = looked_at.0 else {
        return;
    };
    let Ok(button) = buttons.get(entity) else {
        return;
    };

    for child in children.iter_descendants(entity) {
        if let Ok(mut press) = presses.get_mut(child) {
            press.timer.reset();
            press.returning = false;
        }
    }

    if button.trigger.is_empty() {
        return;
    }
    info!("Button pressed: trigger '{}'", button.trigger);
    // TODO: parse button.trigger into ScenarioTrigger
}

fn animate_button_press(time: Res<Time>, mut query: Query<(&mut ButtonPress, &mut Transform)>) {
    for (mut press, mut transform) in &mut query {
        press.timer.tick(time.delta());

        let scale_y = if press.returning {
            let target = 1.0;
            press.current_scale +=
                (target - press.current_scale) * BUTTON_RETURN_SPEED * time.delta_secs();
            if (press.current_scale - target).abs() < 0.01 {
                press.current_scale = target;
            }
            press.current_scale
        } else if press.timer.just_finished() {
            press.returning = true;
            press.current_scale = BUTTON_PRESSED_SCALE;
            BUTTON_PRESSED_SCALE
        } else {
            let t = (press.timer.elapsed_secs() / press.timer.duration().as_secs_f32())
                .clamp(0.0, 1.0);
            let s = 1.0 + (BUTTON_PRESSED_SCALE - 1.0) * t;
            press.current_scale = s;
            s
        };

        transform.scale.y = scale_y;
    }
}
