use bevy::platform::collections::HashMap;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, voxel_sim);
    app.add_systems(Update, remesh_voxels);
}

pub fn voxel_sim(mut sims: Query<&mut VoxelSim>) {
    for mut sim in &mut sims {
        sim.simulate();
    }
}

pub fn remesh_voxels(mut sims: Query<&mut VoxelSim>) {
    // let mut sample_buffers = Vec::new();
    for mut sim in &mut sims {
        // TODO: sample and mesh the voxels if something changed
        // if sim.needs_remesh() {
        // sim.sample()
        // }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Voxel {
    Dirt,
    Sand,
    Barrier,
    Air,
}

#[derive(Component, Clone)]
pub struct VoxelSim {
    bounds: IVec3,
    voxels: Vec<Voxel>,
    entities: HashMap<Voxel, Entity>,
}

impl VoxelSim {
    /// Create a new SimChunks covering chunk coordinates from `min` to `max` (inclusive).
    pub fn new(bounds: IVec3) -> Self {
        Self {
            bounds: bounds,
            voxels: vec![Voxel::Air; (bounds.x * bounds.y * bounds.z) as usize],
            entities: default(),
        }
    }

    pub fn in_bounds(&self, pos: IVec3) -> bool {
        pos.x >= 0
            && pos.x < self.bounds.x
            && pos.y >= 0
            && pos.y < self.bounds.y
            && pos.z >= 0
            && pos.z < self.bounds.z
    }

    pub fn linearize(&self, pos: IVec3) -> usize {
        (pos.z + pos.x * self.bounds.z + pos.y * self.bounds.x * self.bounds.z) as usize
    }

    pub fn delinearize(&self, index: usize) -> IVec3 {
        let index = index as i32;
        let z = index % self.bounds.z;
        let x = (index / self.bounds.z) % self.bounds.x;
        let y = index / (self.bounds.x * self.bounds.z);
        IVec3::new(x, y, z)
    }

    pub fn get(&self, pos: IVec3) -> Option<Voxel> {
        if !self.in_bounds(pos) {
            return None;
        }

        Some(self.voxels[self.linearize(pos)])
    }

    pub fn set(&mut self, pos: IVec3, voxel: Voxel) {
        if !self.in_bounds(pos) {
            return;
        }

        let index = self.linearize(pos);
        self.voxels[index] = voxel;
    }

    pub fn simulate(&mut self) {
        let x_stride = self.linearize(IVec3::X);
        let z_stride = self.linearize(IVec3::Z);
        let y_stride = self.linearize(IVec3::Y);

        for i in 0..self.voxels.len() {
            let voxel = self.voxels[i];
            match voxel {
                Voxel::Dirt => {
                    let below = i.wrapping_sub(y_stride);
                    if let Some(below_voxel) = self.voxels.get(below) {
                        if below_voxel == &Voxel::Air {
                            self.voxels[i] = Voxel::Air;
                            self.voxels[below] = voxel;
                        }
                    }
                }
                Voxel::Sand => {
                    let below = i.wrapping_sub(y_stride);
                    if let Some(below_voxel) = self.voxels.get(below) {
                        if below_voxel == &Voxel::Air {
                            self.voxels[i] = Voxel::Air;
                            self.voxels[below] = voxel;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
