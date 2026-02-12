//! NPC handling. In the demo, the NPC is a fox that moves towards the player. We can interact with the NPC to trigger dialogue.

use animation::NpcAnimationState;
use avian3d::prelude::*;
use bevy::prelude::*;

use bevy_ahoy::CharacterController;
use bevy_trenchbroom::prelude::*;

use bevy::platform::collections::HashMap;

use crate::{
    animation::AnimationState,
    asset_tracking::LoadResource,
    third_party::{
        avian3d::CollisionLayer,
        bevy_trenchbroom::{GetTrenchbroomModelPath, LoadTrenchbroomModel as _},
        bevy_yarnspinner::YarnNode,
    },
};

use super::animation::AnimationPlayerAncestor;
pub(crate) mod ai;
mod animation;
mod assets;
mod sound;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((ai::plugin, animation::plugin, assets::plugin, sound::plugin));
    app.load_asset::<Gltf>(Npc::model_path());
    app.load_asset::<Gltf>("models/crab/scene.gltf");
    app.add_observer(on_add);
    app.init_resource::<NpcRegistry>();
}

#[derive(Clone)]
pub(crate) struct NpcPrefab {
    pub scene: String,
    pub radius: f32,
    pub height: f32,
}

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
            },
        );
        prefabs.insert(
            "crab".into(),
            NpcPrefab {
                scene: "models/crab/scene.gltf#Scene0".into(),
                radius: 0.5,
                height: 0.8,
            },
        );
        Self { prefabs }
    }
}

// #[point_class(base(Transform, Visibility), model("models/fox/Fox.gltf"))]
#[point_class(base(Transform, Visibility), model("models/lobster/lowpoly_lobster.glb"))]
pub(crate) struct Npc;

#[derive(Component)]
pub(crate) struct Body;

#[derive(Component)]
pub(crate) struct Health(pub f32);

pub(crate) const NPC_RADIUS: f32 = 0.6;
pub(crate) const NPC_HEIGHT: f32 = 1.3;
const NPC_HALF_HEIGHT: f32 = NPC_HEIGHT / 2.0;
const NPC_FLOAT_HEIGHT: f32 = NPC_HALF_HEIGHT + 0.01;
const NPC_SPEED: f32 = 7.0;

fn on_add(add: On<Add, Npc>, mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .entity(add.entity)
        .insert((
            Npc,
            Collider::cylinder(NPC_RADIUS, NPC_HEIGHT),
            CharacterController {
                speed: NPC_SPEED,
                ..default()
            },
            ColliderDensity(1_000.0),
            RigidBody::Kinematic,
            // AnimationState::<NpcAnimationState>::default(),
            // AnimationPlayerAncestor,
            CollisionLayers::new(
                CollisionLayer::Character,
                [CollisionLayer::Default, CollisionLayer::Prop],
            ),
            Health(100.0),
            // The Yarn Node is what we use to trigger dialogue.
            YarnNode::new("Lefty_Larry"),
        ))
        .with_child((
            Name::new("Npc Model"),
            SceneRoot(assets.load_trenchbroom_model::<Npc>()),
            Transform::from_xyz(0.0, 0.0, 0.0)
                .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
        ));
        // .observe(setup_npc_animations);
}
