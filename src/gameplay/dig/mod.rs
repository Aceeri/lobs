use avian3d::prelude::{Collider, RigidBody};
use bevy::asset::RenderAssetUsages;
use bevy::math::DVec3;
use bevy::mesh::PrimitiveTopology;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_trenchbroom::brush::ConvexHull;
use bevy_trenchbroom::geometry::{Brushes, BrushesAsset};
use bevy_trenchbroom::prelude::*;
use fast_surface_nets::ndshape::{RuntimeShape, Shape};
use fast_surface_nets::{SurfaceNetsBuffer, surface_nets};
use fixedbitset::FixedBitSet;

/// World-space size of a single voxel. 4 voxels per world unit.
pub const VOXEL_SIZE: f32 = 0.25;

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, voxel_sim);
    app.add_systems(Update, (remesh_voxels, init_voxel_volumes));
    app.add_observer(add_dirty_buff);
    app.add_observer(add_voxel_children);
}

#[derive(FgdType, Reflect, Debug, Clone, Default)]
#[number_key]
pub enum VoxelFill {
    #[default]
    /// Dirt
    Dirt = 0,
    /// Sand
    Sand = 1,
}

#[solid_class(base(Transform, Visibility))]
pub(crate) struct VoxelVolume {
    pub fill: VoxelFill,
}

impl Default for VoxelVolume {
    fn default() -> Self {
        Self {
            fill: VoxelFill::default(),
        }
    }
}

fn init_voxel_volumes(
    mut commands: Commands,
    volumes: Query<(Entity, &VoxelVolume, &Brushes), Without<VoxelSim>>,
    brushes_assets: Res<Assets<BrushesAsset>>,
) {
    for (entity, volume, brushes) in &volumes {
        let brushes_asset = match brushes {
            Brushes::Owned(asset) => asset,
            Brushes::Shared(handle) => {
                let Some(asset) = brushes_assets.get(handle) else {
                    continue;
                };
                asset
            }
            #[allow(unreachable_patterns)]
            _ => continue,
        };

        let mut min = DVec3::INFINITY;
        let mut max = DVec3::NEG_INFINITY;
        for brush in brushes_asset.iter() {
            if let Some((from, to)) = brush.as_cuboid() {
                min = min.min(from);
                max = max.max(to);
            } else {
                for (vertex, _) in brush.calculate_vertices() {
                    min = min.min(vertex);
                    max = max.max(vertex);
                }
            }
        }

        if !min.is_finite() || !max.is_finite() {
            continue;
        }

        let size = max - min;
        let voxels_per_unit = (1.0 / VOXEL_SIZE) as f64;
        let bounds = IVec3::new(
            (size.x * voxels_per_unit).ceil() as i32,
            (size.y * voxels_per_unit).ceil() as i32,
            (size.z * voxels_per_unit).ceil() as i32,
        )
        .max(IVec3::ONE);

        let mut sim = VoxelSim::new(bounds);

        let voxel = match volume.fill {
            VoxelFill::Dirt => Voxel::Dirt,
            VoxelFill::Sand => Voxel::Sand,
        };

        // just fill it
        for x in 0..bounds.x {
            for z in 0..bounds.z {
                for y in 0..bounds.y {
                    sim.set(IVec3::new(x, y, z), voxel);
                }
            }
        }

        // center the voxel mesh on the brush AABB, should align it ok with trenchbroom
        let aabb_center = ((min + max) * 0.5).as_vec3();
        let mesh_center =
            Vec3::new(bounds.x as f32, bounds.y as f32, bounds.z as f32) * VOXEL_SIZE * 0.5;
        let translation = aabb_center - mesh_center;
        commands.entity(entity).insert((
            sim,
            RigidBody::Static,
            Transform::from_translation(translation),
        ));
    }
}

pub fn voxel_sim(mut sims: Query<(&mut VoxelSim, &mut DirtyBuffer)>) {
    for (mut sim, mut dirty) in &mut sims {
        sim.simulate(&mut *dirty);
    }
}

pub fn remesh_voxels(
    mut commands: Commands,
    mut sims: Query<(Entity, &mut VoxelSim, &VoxelEntities)>,
    mut mesh3ds: Query<&mut Mesh3d>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (sim_entity, mut sim, entities) in &mut sims {
        if !sim.needs_remesh {
            continue;
        }
        sim.needs_remesh = false;

        let buffers = sim.sample();
        for (voxel, buffer) in &buffers {
            let Some(&entity) = entities.entities.get(voxel) else {
                continue;
            };
            let Ok(mut mesh3d) = mesh3ds.get_mut(entity) else {
                continue;
            };
            let mesh = build_flat_mesh(&buffer);
            mesh3d.0 = meshes.add(mesh);
        }

        // voxel collider from all non-air positions
        let mut voxel_positions: Vec<IVec3> = Vec::new();
        for i in 0..sim.voxels.len() {
            if sim.voxels[i] != Voxel::Air {
                voxel_positions.push(sim.delinearize(i));
            }
        }
        if !voxel_positions.is_empty() {
            commands
                .entity(sim_entity)
                .insert(Collider::voxels(Vec3::splat(VOXEL_SIZE), &voxel_positions));
        }
    }
}

/// Texture scale: how many world units per full texture repeat.
const UV_SCALE: f32 = 1.0;

fn build_flat_mesh(buffer: &SurfaceNetsBuffer) -> Mesh {
    let num_tris = buffer.indices.len() / 3;
    let mut positions = Vec::with_capacity(num_tris * 3);
    let mut normals = Vec::with_capacity(num_tris * 3);
    let mut uvs = Vec::with_capacity(num_tris * 3);

    for tri in 0..num_tris {
        let i0 = buffer.indices[tri * 3] as usize;
        let i1 = buffer.indices[tri * 3 + 1] as usize;
        let i2 = buffer.indices[tri * 3 + 2] as usize;

        let p0 = Vec3::from(buffer.positions[i0]);
        let p1 = Vec3::from(buffer.positions[i1]);
        let p2 = Vec3::from(buffer.positions[i2]);

        let face_normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();
        let n = face_normal.to_array();

        // scuffed triplanar mapping
        // just take the best normal direction and take the uv related to that plane
        // e.g. a high y means xz, a high z means yx, a high x means yz
        let abs_n = face_normal.abs();
        for p in [p0, p1, p2] {
            positions.push(p.to_array());
            normals.push(n);
            let uv = if abs_n.x >= abs_n.y && abs_n.x >= abs_n.z {
                // high x, yz plane
                [p.y / UV_SCALE, p.z / UV_SCALE]
            } else if abs_n.y >= abs_n.z && abs_n.y >= abs_n.x {
                // high y, xz plane
                [p.x / UV_SCALE, p.z / UV_SCALE]
            } else {
                // high z, xy plane
                [p.x / UV_SCALE, p.y / UV_SCALE]
            };
            uvs.push(uv);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
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

pub fn add_voxel_children(
    on: On<Add, VoxelEntities>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sim: Query<&mut VoxelEntities>,
    assets: Res<AssetServer>,
) {
    let Ok(mut entities) = sim.get_mut(on.entity) else {
        return;
    };

    for voxel in &[Voxel::Sand, Voxel::Dirt] {
        let material =
            match voxel {
                Voxel::Dirt => StandardMaterial {
                    base_color_texture: Some(
                        assets.load("textures/darkmod/nature/dirt/dirt_002_dark.png"),
                    ),
                    normal_map_texture: Some(assets.load(
                        "textures/darkmod/nature/dirt/dirt_002_dark/dirt_002_dark_normal.png",
                    )),
                    perceptual_roughness: 0.9,
                    reflectance: 0.2,
                    ..default()
                },
                Voxel::Sand => StandardMaterial {
                    base_color: Color::srgb(0.8, 0.8, 0.8),
                    perceptual_roughness: 1.0,
                    reflectance: 0.2,
                    ..default()
                },
                _ => continue,
            };

        let voxel_id = commands
            .spawn((
                Name::new(format!("Voxel {:?}", voxel)),
                Transform::default(),
                MeshMaterial3d(materials.add(material)),
                Mesh3d(default()),
                ChildOf(on.entity),
            ))
            .id();
        entities.entities.insert(*voxel, voxel_id);
    }
}

#[derive(Component, Clone)]
#[require(VoxelEntities)]
pub struct VoxelSim {
    bounds: IVec3,
    voxels: Vec<Voxel>,
    modified: FixedBitSet,
    needs_remesh: bool,
}

impl VoxelSim {
    pub fn new(bounds: IVec3) -> Self {
        let volume = (bounds.x * bounds.y * bounds.z) as usize;
        Self {
            bounds,
            voxels: vec![Voxel::Air; volume],
            modified: FixedBitSet::with_capacity(volume),
            needs_remesh: false,
        }
    }

    fn volume(&self) -> usize {
        (self.bounds.x * self.bounds.y * self.bounds.z) as usize
    }

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
        self.needs_remesh = true;
    }

    pub fn sample(&self) -> HashMap<Voxel, SurfaceNetsBuffer> {
        // +1 padding on min side, +2 on max side.
        // surface_nets doesn't generate faces on the positive boundary,
        // so we need the extra layer on max to avoid missing quads there.
        let padded = [
            self.bounds.x as u32 + 3,
            self.bounds.y as u32 + 3,
            self.bounds.z as u32 + 3,
        ];
        let shape = RuntimeShape::<u32, 3>::new(padded);
        let max = [padded[0] - 1, padded[1] - 1, padded[2] - 1];
        let num_samples = (padded[0] * padded[1] * padded[2]) as usize;

        let mut results = HashMap::new();
        for &voxel_type in &[Voxel::Sand, Voxel::Dirt] {
            let mut sdf = vec![0.5f32; num_samples];
            for i in 0..self.voxels.len() {
                if self.voxels[i] == voxel_type {
                    let pos = self.delinearize(i);
                    let sdf_index = Shape::linearize(
                        &shape,
                        [pos.x as u32 + 1, pos.y as u32 + 1, pos.z as u32 + 1],
                    ) as usize;
                    sdf[sdf_index] = -0.5;
                }
            }
            let mut buffer = SurfaceNetsBuffer::default();
            surface_nets(&sdf, &shape, [0; 3], max, &mut buffer);
            for p in &mut buffer.positions {
                p[0] = (p[0] - 0.5) * VOXEL_SIZE;
                p[1] = (p[1] - 0.5) * VOXEL_SIZE;
                p[2] = (p[2] - 0.5) * VOXEL_SIZE;
            }
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
                        self.needs_remesh = true;
                    }
                }
                _ => {}
            }
        }
    }
}
