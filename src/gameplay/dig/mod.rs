


// Super simplified version of voxel storage
pub struct SimChunks {
    chunks: HashMap<IVec3, [Voxel; 16*16*16]>,
}
