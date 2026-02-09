use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use fast_surface_nets::ndshape::{RuntimeShape, Shape};
use fast_surface_nets::{SurfaceNetsBuffer, surface_nets};
use fixedbitset::FixedBitSet;

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, voxel_sim);
    app.add_systems(Update, remesh_voxels);
    app.add_observer(add_dirty_buff);
}

pub fn voxel_sim(mut sims: Query<(&mut VoxelSim, &mut DirtyBuffer)>) {
    for (mut sim, mut dirty) in &mut sims {
        sim.simulate(&mut *dirty);
    }
}

pub fn remesh_voxels(mut sims: Query<(&mut VoxelSim, &mut VoxelEntities)>) {
    for (mut sim, mut entities) in &mut sims {
        if sim.any_modified() {
            let buffers = sim.sample();
            for (voxel, buffer) in buffers {}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Voxel {
    Dirt,
    Sand,
    Barrier,
    Air,
}

/// 18-connected neighbor offsets (6 face + 12 edge neighbors).
const NEIGHBORS_18: [IVec3; 18] = [
    // face neighbors
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
    // edge neighbors
    IVec3::new(1, 1, 0),
    IVec3::new(-1, 1, 0),
    IVec3::new(1, -1, 0),
    IVec3::new(-1, -1, 0),
    IVec3::new(1, 0, 1),
    IVec3::new(-1, 0, 1),
    IVec3::new(1, 0, -1),
    IVec3::new(-1, 0, -1),
    IVec3::new(0, 1, 1),
    IVec3::new(0, -1, 1),
    IVec3::new(0, 1, -1),
    IVec3::new(0, -1, -1),
];

#[inline]
pub fn linearize(bounds: IVec3, pos: IVec3) -> usize {
    (pos.z + pos.x * bounds.z + pos.y * bounds.x * bounds.z) as usize
}

#[inline]
pub fn delinearize(bounds: IVec3, index: usize) -> IVec3 {
    let index = index as i32;
    let z = index % bounds.z;
    let x = (index / bounds.z) % bounds.x;
    let y = index / (bounds.x * bounds.z);
    IVec3::new(x, y, z)
}

#[inline]
pub fn in_bounds(bounds: IVec3, pos: IVec3) -> bool {
    pos.x >= 0
        && pos.x < bounds.x
        && pos.y >= 0
        && pos.y < bounds.y
        && pos.z >= 0
        && pos.z < bounds.z
}

#[derive(Component, Clone)]
pub struct DirtyBuffer {
    bounds: IVec3,
    dirty: FixedBitSet,
}

impl DirtyBuffer {
    pub fn new(bounds: IVec3) -> Self {
        Self {
            bounds: bounds,
            dirty: FixedBitSet::with_capacity((bounds.x * bounds.y * bounds.z) as usize),
        }
    }

    pub fn linearize(&self, pos: IVec3) -> usize {
        linearize(self.bounds, pos)
    }

    pub fn delinearize(&self, index: usize) -> IVec3 {
        delinearize(self.bounds, index)
    }

    pub fn in_bounds(&self, pos: IVec3) -> bool {
        in_bounds(self.bounds, pos)
    }

    pub fn dilate_modified(&mut self, modified: &FixedBitSet) {
        for index in modified.ones() {
            let pos = self.delinearize(index);
            for offset in &NEIGHBORS_18 {
                let neighbor = pos + *offset;
                if self.in_bounds(neighbor) {
                    self.dirty.insert(self.linearize(neighbor));
                }
            }
        }
    }
}

#[derive(Component, Clone, Default)]
pub struct VoxelEntities {
    entities: HashMap<Voxel, Entity>,
}

pub fn add_dirty_buff(on: On<Add, VoxelSim>, mut commands: Commands, sim: Query<&VoxelSim>) {
    let Ok(sim) = sim.get(on.entity) else {
        return;
    };

    commands
        .entity(on.entity)
        .insert(DirtyBuffer::new(sim.bounds));
}

#[derive(Component, Clone)]
#[require(VoxelEntities)]
pub struct VoxelSim {
    bounds: IVec3,
    voxels: Vec<Voxel>,
    modified: FixedBitSet,
}

impl VoxelSim {
    pub fn new(bounds: IVec3) -> Self {
        let volume = (bounds.x * bounds.y * bounds.z) as usize;
        Self {
            bounds,
            voxels: vec![Voxel::Air; volume],
            modified: FixedBitSet::with_capacity(volume),
        }
    }

    fn volume(&self) -> usize {
        (self.bounds.x * self.bounds.y * self.bounds.z) as usize
    }

    /// Mark a cell and its 18-connected neighbors as dirty in the write buffer.
    fn mark_modified(&mut self, index: usize) {
        self.modified.insert(index);
    }

    fn any_modified(&self) -> bool {
        !self.modified.is_clear()
    }

    pub fn linearize(&self, pos: IVec3) -> usize {
        linearize(self.bounds, pos)
    }

    pub fn delinearize(&self, index: usize) -> IVec3 {
        delinearize(self.bounds, index)
    }

    pub fn in_bounds(&self, pos: IVec3) -> bool {
        in_bounds(self.bounds, pos)
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
        self.mark_modified(index);
    }

    pub fn sample(&self) -> HashMap<Voxel, SurfaceNetsBuffer> {
        let padded = [
            self.bounds.x as u32 + 2,
            self.bounds.y as u32 + 2,
            self.bounds.z as u32 + 2,
        ];
        let shape = RuntimeShape::<u32, 3>::new(padded);
        let max = [padded[0] - 1, padded[1] - 1, padded[2] - 1];
        let num_samples = (padded[0] * padded[1] * padded[2]) as usize;

        let mut results = HashMap::new();
        for &voxel_type in &[Voxel::Sand, Voxel::Dirt] {
            let mut sdf = vec![-0.5f32; num_samples];
            for i in 0..self.voxels.len() {
                if self.voxels[i] == voxel_type {
                    let pos = self.delinearize(i);
                    let sdf_index = Shape::linearize(
                        &shape,
                        [pos.x as u32 + 1, pos.y as u32 + 1, pos.z as u32 + 1],
                    ) as usize;
                    sdf[sdf_index] = 0.5;
                }
            }
            let mut buffer = SurfaceNetsBuffer::default();
            surface_nets(&sdf, &shape, [0; 3], max, &mut buffer);
            results.insert(voxel_type, buffer);
        }
        results
    }

    pub fn simulate(&mut self, dirty: &mut DirtyBuffer) {
        let y_stride = self.linearize(IVec3::Y);
        let volume = self.volume();

        dirty.dirty.clear();
        dirty.dilate_modified(&self.modified);
        self.modified.clear();

        for i in dirty.dirty.ones() {
            let voxel = self.voxels[i];
            match voxel {
                Voxel::Dirt | Voxel::Sand => {
                    let below = i.wrapping_sub(y_stride);
                    if below < volume && self.voxels[below] == Voxel::Air {
                        self.voxels[i] = Voxel::Air;
                        self.voxels[below] = voxel;

                        self.mark_modified(i);
                        self.mark_modified(below);
                    }
                }
                _ => {}
            }
        }
    }
}
