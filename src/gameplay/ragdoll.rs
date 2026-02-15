//! Automated ragdoll generation (kinda shit)
//!
//! Basic idea: take a "core" amount of joints and "external joints" and just create groups of joints with those. This should make some 'okay' ragdolls while keeping stability
//! For future improvements: maybe some graph theory nonsense or distance heuristics?
//!
//! The goal is mainly to make it obvious to the player that something is `dead` and draggable.

use bevy::{
    mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    prelude::*,
};

struct Ragdoll {
    core: Vec<Entity>,
    external: Vec<Vec<Entity>>,
}

impl Ragdoll {
    pub fn from_skinned(
        skinned_mesh: &SkinnedMesh,
        bindposes: &Assets<SkinnedMeshInverseBindposes>,
        globals: &Query<&GlobalTransform>,
    ) -> Option<Self> {
        let bindposes = bindposes.get(&skinned_mesh.inverse_bindposes)?;
        for (joint_entity, bindpose) in skinned_mesh.joints.iter().zip(bindposes.iter()) {
            let global = globals
                .get(*joint_entity)
                .expect("joint should have a GlobalTransform");
        }

        None
    }
}
// grab N number of joints from the "root" of the ragdoll and make that the `core`
fn core(available_joints: &mut Vec<Entity>, globals: &Query<&GlobalTransform>) -> Vec<Entity> {
    vec![]
}

// grab remaining graph chains that connect any available joints, combine those into remaining joints
fn external(available_joints: &mut Vec<Entity>, globals: &Query<&GlobalTransform>) -> Vec<Entity> {
    vec![]
}
