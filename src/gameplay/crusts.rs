use std::iter;

use bevy::{
    camera::{RenderTarget, primitives::Aabb, visibility::RenderLayers},
    core_pipeline::prepass::DepthPrepass,
    prelude::*,
    render::render_resource::TextureFormat,
    scene::SceneInstanceReady,
    ui::widget::ViewportNode,
};

use crate::{RenderLayer, asset_tracking::LoadResource, screens::Screen};

// hacky shit, should probably just have separate render layers or a closer `far` or something
const PREVIEW_SPACING: f32 = 100.0;
const PREVIEW_BASE_Y: f32 = -1000.0;

#[derive(Component)]
pub struct SpinningPreview {
    pub speed: f32,
}

#[derive(Component)]
pub struct PreviewModel;

#[derive(Component)]
pub struct PreviewCamera {
    model: Entity,
    offset: Vec3,
}

pub struct PreviewEntities {
    pub camera: Entity,
    pub model: Entity,
}

// TODO: move this shit into its own file
pub fn spawn_model_preview(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    scene: Handle<Scene>,
    index: usize,
    spin_speed: f32,
    model_transform: Transform,
    label: &str,
) -> PreviewEntities {
    let offset = Vec3::new(0.0, PREVIEW_BASE_Y + index as f32 * PREVIEW_SPACING, 0.0);

    let image = Image::new_target_texture(128, 128, TextureFormat::Bgra8UnormSrgb, None);
    let image_handle = images.add(image);

    let scene_child = commands
        .spawn((
            Name::new(format!("Preview Model ({label})")),
            PreviewModel,
            SceneRoot(scene),
            model_transform,
            RenderLayers::from(RenderLayer::CRAB_HUD),
        ))
        .id();

    let spinner_entity = commands
        .spawn((
            Name::new("Preview Spinner"),
            SpinningPreview { speed: spin_speed },
            Transform::from_translation(offset),
            Visibility::Inherited,
            RenderLayers::from(RenderLayer::CRAB_HUD),
            DespawnOnExit(Screen::Gameplay),
        ))
        .add_child(scene_child)
        .id();

    let camera_entity = commands
        .spawn((
            Name::new("Preview Camera"),
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                far: 10.0, // clip close
                near: 0.01,
                ..default()
            }),
            Camera {
                order: 0,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            AmbientLight {
                color: Color::WHITE,
                brightness: 1000.0,
                ..default()
            },
            Msaa::Off,
            DepthPrepass,
            RenderTarget::Image(image_handle.into()),
            Transform::from_translation(offset + Vec3::new(0.0, 0.5, 3.0))
                .looking_at(offset, Vec3::Y),
            RenderLayers::from(RenderLayer::CRAB_HUD),
            PreviewCamera {
                model: spinner_entity,
                offset,
            },
            DespawnOnExit(Screen::Gameplay),
        ))
        .id();

    // commands.spawn((
    //     Name::new("Preview Light"),
    //     PointLight {
    //         intensity: 5000.0,
    //         shadows_enabled: false,
    //         range: 20.0,
    //         ..default()
    //     },
    //     Transform::from_translation(offset + Vec3::new(2.0, 3.0, 2.0)),
    //     RenderLayers::from(RenderLayer::CRAB_HUD),
    //     DespawnOnExit(Screen::Gameplay),
    // ));

    PreviewEntities {
        camera: camera_entity,
        model: scene_child,
    }
}

/// Assign the preview render layer to all mesh descendants once a scene is ready.
fn configure_preview_render_layers(
    ready: On<SceneInstanceReady>,
    mut commands: Commands,
    q_preview: Query<(), With<PreviewModel>>,
    q_children: Query<&Children>,
    q_mesh: Query<(), With<Mesh3d>>,
) {
    let root = ready.entity;
    if !q_preview.contains(root) {
        return;
    }

    for child in iter::once(root)
        .chain(q_children.iter_descendants(root))
        .filter(|e| q_mesh.contains(*e))
    {
        commands
            .entity(child)
            .insert(RenderLayers::from(RenderLayer::CRAB_HUD));
    }
}

/// Position preview cameras at 2x the model's largest AABB extent on Z.
fn position_preview_cameras(
    mut cameras: Query<(&PreviewCamera, &mut Transform)>,
    q_children: Query<&Children>,
    q_preview_model: Query<Entity, With<PreviewModel>>,
    q_aabb: Query<&Aabb>,
) {
    for (preview, mut cam_transform) in &mut cameras {
        let Ok(children) = q_children.get(preview.model) else {
            continue;
        };
        let Some(model_entity) = children.iter().find(|e| q_preview_model.contains(*e)) else {
            continue;
        };

        let mut max_extent: f32 = 0.0;
        let mut found = false;

        for descendant in iter::once(model_entity).chain(q_children.iter_descendants(model_entity))
        {
            let Ok(aabb) = q_aabb.get(descendant) else {
                continue;
            };
            max_extent = max_extent.max(aabb.half_extents.max_element());
            found = true;
        }

        if !found {
            continue;
        }

        let dist = max_extent.max(0.2) * 2.0;
        *cam_transform = Transform::from_translation(preview.offset + Vec3::new(0.0, 0.0, dist))
            .looking_at(preview.offset, Vec3::Y);
    }
}

fn spin_previews(mut query: Query<(&mut Transform, &SpinningPreview)>, time: Res<Time>) {
    for (mut transform, preview) in &mut query {
        transform.rotate_y(preview.speed * time.delta_secs());
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Crusts>();
    app.load_resource::<CrustsAssets>();
    app.add_systems(OnEnter(Screen::Gameplay), spawn_crusts_hud);
    app.add_systems(
        Update,
        (
            spin_previews,
            position_preview_cameras,
            update_crusts_text.run_if(resource_changed::<Crusts>),
        ),
    );
    app.add_observer(configure_preview_render_layers);
}

// TODO: make this a per player thing when we add coop
#[derive(Resource, Default)]
pub(crate) struct Crusts(pub(crate) u32);

impl Crusts {
    pub fn add(&mut self, amount: u32) {
        self.0 += amount;
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
struct CrustsAssets {
    #[dependency]
    crab: Handle<Scene>,
}

impl FromWorld for CrustsAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            crab: assets.load("models/crab/scene.gltf#Scene0"),
        }
    }
}

#[derive(Component)]
struct CrustsCounterText;

fn spawn_crusts_hud(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    crusts_assets: Res<CrustsAssets>,
    crusts: Res<Crusts>,
) {
    let preview = spawn_model_preview(
        &mut commands,
        &mut images,
        crusts_assets.crab.clone(),
        0,
        0.5,
        Transform::from_rotation(Quat::from_rotation_x(1.57)),
        "Crab",
    );

    let red_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.1, 0.1),
        ..default()
    });
    commands.entity(preview.model).observe(
        move |ready: On<SceneInstanceReady>,
              mut commands: Commands,
              q_children: Query<&Children>,
              q_mesh: Query<(), With<Mesh3d>>| {
            let root = ready.entity;
            for child in iter::once(root)
                .chain(q_children.iter_descendants(root))
                .filter(|e| q_mesh.contains(*e))
            {
                commands
                    .entity(child)
                    .insert(MeshMaterial3d(red_mat.clone()));
            }
        },
    );

    commands
        .spawn((
            Name::new("Crusts HUD"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(Screen::Gameplay),
        ))
        .with_children(|parent| {
            parent
                .spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ViewportNode::new(preview.camera),
                        Node {
                            width: Val::Px(48.0),
                            height: Val::Px(48.0),
                            ..default()
                        },
                    ));
                    row.spawn((
                        CrustsCounterText,
                        Text::new(format!("{}", crusts.0)),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

fn update_crusts_text(crusts: Res<Crusts>, mut query: Query<&mut Text, With<CrustsCounterText>>) {
    for mut text in &mut query {
        **text = format!("{}", crusts.0);
    }
}
