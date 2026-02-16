//! Per-joint ragdoll: one rigid body per skeleton joint with convex hull
//! colliders built from mesh vertices grouped by joint weight assignment.
//!
//! Joints are deparented from the skeleton hierarchy so Bevy's transform
//! propagation doesn't interfere — physics joints (constraints) drive their
//! positions instead.

use avian3d::prelude::*;
use bevy::{
    mesh::{VertexAttributeValues, skinning::SkinnedMesh},
    platform::collections::HashMap,
    prelude::*,
};

use super::grave::Slotted;
use super::npc::Body;
use crate::third_party::avian3d::CollisionLayer;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (create_ragdolls, ragdoll_writeback, freeze_ragdoll_on_slot),
    );
}

#[derive(Component)]
pub(crate) struct RagdollRequest;

#[derive(Component, Clone)]
pub(crate) struct RagdollConfig {
    pub fallback_radius: f32,
    pub swing_limit: f32,
    pub twist_limit: f32,
    pub damping: f32,
}

impl Default for RagdollConfig {
    fn default() -> Self {
        Self {
            fallback_radius: 0.05,
            swing_limit: 0.8,
            twist_limit: 0.4,
            damping: 2.0,
        }
    }
}

#[derive(Component)]
pub(crate) struct RagdollCore;

#[derive(Component)]
struct RagdollJointBody {
    joint_entity: Entity,
    core: Entity,
}

#[derive(Component)]
struct DeparentedJoint;

const RAGDOLL_DENSITY: f32 = 500.0;

/// Groups mesh vertices by their primary (highest-weight) joint index.
fn extract_vertices_per_joint(mesh: &Mesh) -> Option<HashMap<usize, Vec<Vec3>>> {
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(v) => v,
        _ => return None,
    };
    let joint_indices_raw = mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX)?;
    let joint_weights = match mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)? {
        VertexAttributeValues::Float32x4(v) => v,
        _ => return None,
    };

    let mut map: HashMap<usize, Vec<Vec3>> = HashMap::new();

    for (i, pos) in positions.iter().enumerate() {
        let weights = &joint_weights[i];

        // Find which influence slot has the highest weight
        let mut best_slot = 0;
        let mut best_weight = weights[0];
        for slot in 1..4 {
            if weights[slot] > best_weight {
                best_weight = weights[slot];
                best_slot = slot;
            }
        }

        let joint_index = match joint_indices_raw {
            VertexAttributeValues::Uint16x4(v) => v[i][best_slot] as usize,
            VertexAttributeValues::Uint8x4(v) => v[i][best_slot] as usize,
            _ => continue,
        };

        map.entry(joint_index)
            .or_default()
            .push(Vec3::from_array(*pos));
    }

    Some(map)
}

fn create_ragdolls(
    mut commands: Commands,
    ragdoll_requests: Query<(Entity, Option<&RagdollConfig>), With<RagdollRequest>>,
    children_query: Query<&Children>,
    parents: Query<&ChildOf>,
    skinned_meshes: Query<(Entity, &SkinnedMesh)>,
    mesh_handles: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
    globals: Query<&GlobalTransform>,
) {
    for (npc_entity, config) in &ragdoll_requests {
        // Find skinned mesh entity
        let Some((mesh_entity, skinned)) =
            find_skinned_mesh_entity(npc_entity, &children_query, &skinned_meshes)
        else {
            continue;
        };

        let joints = &skinned.joints;
        if joints.is_empty() {
            commands
                .entity(npc_entity)
                .remove::<(RagdollRequest, RagdollConfig)>();
            continue;
        }

        // Read mesh data (skip if mesh not loaded yet — retry next frame)
        let Ok(mesh_handle) = mesh_handles.get(mesh_entity) else {
            continue;
        };
        let Some(mesh) = meshes.get(&mesh_handle.0) else {
            continue;
        };

        // Extract vertices grouped by primary joint
        let Some(vertices_per_joint) = extract_vertices_per_joint(mesh) else {
            commands
                .entity(npc_entity)
                .remove::<(RagdollRequest, RagdollConfig)>();
            continue;
        };

        let config = config.cloned().unwrap_or_default();
        let mesh_global = globals.get(mesh_entity).copied().unwrap_or_default();

        // Capture all joint world transforms before any modifications
        struct CapturedJoint {
            translation: Vec3,
            rotation: Quat,
            scale: Vec3,
        }
        let captured: Vec<CapturedJoint> = joints
            .iter()
            .map(|&j| {
                let gt = globals.get(j).copied().unwrap_or_default();
                let (scale, rotation, translation) = gt.to_scale_rotation_translation();
                CapturedJoint {
                    translation,
                    rotation,
                    scale,
                }
            })
            .collect();

        // Build joint-index lookup (entity → index in joints array)
        let joint_set: HashMap<Entity, usize> =
            joints.iter().enumerate().map(|(i, &e)| (e, i)).collect();

        // Find root joint (whose parent is not in the joint set)
        let Some(root_idx) = joints.iter().enumerate().find_map(|(i, &j)| {
            if parents
                .get(j)
                .map_or(true, |p| !joint_set.contains_key(&p.0))
            {
                Some(i)
            } else {
                None
            }
        }) else {
            commands
                .entity(npc_entity)
                .remove::<(RagdollRequest, RagdollConfig)>();
            continue;
        };

        // Build skeleton parent map: child_index → parent_index
        let parent_map: HashMap<usize, usize> = joints
            .iter()
            .enumerate()
            .filter_map(|(i, &j)| {
                parents
                    .get(j)
                    .ok()
                    .and_then(|p| joint_set.get(&p.0).map(|&pi| (i, pi)))
            })
            .collect();

        let collision_layers = CollisionLayers::new(
            CollisionLayer::Ragdoll,
            [
                CollisionLayer::Level,
                CollisionLayer::Prop,
                CollisionLayer::Sensor,
            ],
        );

        // Spawn one rigid body per joint
        let mut joint_bodies: Vec<Entity> = Vec::with_capacity(joints.len());
        let mut core_entity = Entity::PLACEHOLDER;

        for (idx, _) in joints.iter().enumerate() {
            let joint_world_pos = captured[idx].translation;

            // Build collider from vertices assigned to this joint
            let collider = if let Some(verts) = vertices_per_joint.get(&idx) {
                // Transform mesh-local vertices to world space, then offset from joint
                let offsets: Vec<Vec3> = verts
                    .iter()
                    .map(|&v| mesh_global.transform_point(v) - joint_world_pos)
                    .collect();

                if offsets.len() >= 4 {
                    Collider::convex_hull(offsets)
                        .unwrap_or_else(|| Collider::sphere(config.fallback_radius))
                } else {
                    Collider::sphere(config.fallback_radius)
                }
            } else {
                Collider::sphere(config.fallback_radius)
            };

            let body = commands
                .spawn((
                    RigidBody::Dynamic,
                    collider,
                    ColliderDensity(RAGDOLL_DENSITY),
                    collision_layers.clone(),
                    Transform::from_translation(joint_world_pos),
                ))
                .id();

            if idx == root_idx {
                commands.entity(body).insert((RagdollCore, Body));
                core_entity = body;
            }

            joint_bodies.push(body);
        }

        // Insert RagdollJointBody on every body (now that core_entity is known)
        for (idx, &body) in joint_bodies.iter().enumerate() {
            commands.entity(body).insert(RagdollJointBody {
                joint_entity: joints[idx],
                core: core_entity,
            });
        }

        // Create SphericalJoints between parent→child pairs
        for (&child_idx, &parent_idx) in &parent_map {
            let parent_body = joint_bodies[parent_idx];
            let child_body = joint_bodies[child_idx];

            // Anchor on parent: offset from parent joint to child joint (world-aligned at spawn)
            let parent_anchor = captured[child_idx].translation - captured[parent_idx].translation;

            commands.spawn((
                SphericalJoint::new(parent_body, child_body)
                    .with_local_anchor1(parent_anchor)
                    .with_local_anchor2(Vec3::ZERO)
                    .with_swing_limits(-config.swing_limit, config.swing_limit)
                    .with_twist_limits(-config.twist_limit, config.twist_limit),
                JointDamping {
                    linear: config.damping,
                    angular: config.damping,
                },
            ));
        }

        // Deparent all joints — set Transform to captured world values so
        // GlobalTransform == Transform (no parent) and skinning still works.
        for (idx, &joint_entity) in joints.iter().enumerate() {
            let cap = &captured[idx];
            commands.entity(joint_entity).remove::<ChildOf>().insert((
                DeparentedJoint,
                Transform {
                    translation: cap.translation,
                    rotation: cap.rotation,
                    scale: cap.scale,
                },
            ));
        }

        // Cleanup NPC entity
        commands.entity(npc_entity).remove::<(
            RagdollRequest,
            RagdollConfig,
            Collider,
            RigidBody,
            CollisionLayers,
        )>();
    }
}

/// Copies physics body positions/rotations back to deparented skeleton joints.
fn ragdoll_writeback(
    bodies: Query<(&RagdollJointBody, &Position, &Rotation)>,
    mut transforms: Query<&mut Transform>,
) {
    for (body, position, rotation) in &bodies {
        if let Ok(mut transform) = transforms.get_mut(body.joint_entity) {
            transform.translation = position.0;
            transform.rotation = rotation.0;
        }
    }
}

/// When the core body gets slotted in a grave, freeze all bodies in the ragdoll.
fn freeze_ragdoll_on_slot(
    mut commands: Commands,
    slotted_cores: Query<Entity, (With<RagdollCore>, Added<Slotted>)>,
    bodies: Query<(Entity, &RagdollJointBody)>,
) {
    for core_entity in &slotted_cores {
        for (body_entity, body) in &bodies {
            if body.core == core_entity {
                commands.entity(body_entity).insert(RigidBody::Static);
            }
        }
    }
}

fn find_skinned_mesh_entity<'a>(
    entity: Entity,
    children_query: &Query<&Children>,
    skinned_meshes: &'a Query<(Entity, &SkinnedMesh)>,
) -> Option<(Entity, &'a SkinnedMesh)> {
    if let Ok(result) = skinned_meshes.get(entity) {
        return Some(result);
    }
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            if let Some(found) = find_skinned_mesh_entity(child, children_query, skinned_meshes) {
                return Some(found);
            }
        }
    }
    None
}
