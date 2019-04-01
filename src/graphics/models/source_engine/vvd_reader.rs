use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::mem;

use crate::copy_c_struct;

// https://developer.valvesoftware.com/wiki/VVD

#[derive(Debug)]
pub struct VVDDeserializeError {
    details: String,
}

impl VVDDeserializeError {
    fn new(msg: &str) -> VVDDeserializeError {
        VVDDeserializeError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for VVDDeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for VVDDeserializeError {
    fn description(&self) -> &str {
        &self.details
    }
}

// >>> import struct
// >>> struct.unpack("<i", "\x49\x44\x53\x56")
// (1448297545,)
const VVD_HEADER: i32 = 1448297545;

// https://github.com/ValveSoftware/source-sdk-2013/blob/master/sp/src/public/studio.h
const MAX_NUM_LODS: usize = 8;

// these structures can be found in <mod folder>/src/public/studio.h
#[derive(Copy, Clone)]
pub struct VVDFileHeader {
    pub id: i32,                               // MODEL_VERTEX_FILE_ID
    pub version: i32,                          // MODEL_VERTEX_FILE_VERSION
    pub checksum: i32,                         // same as studiohdr_t, ensures sync
    pub num_lods: i32,                         // num of valid lods
    pub num_lod_vertexes: [i32; MAX_NUM_LODS], // num verts for desired root lod
    pub num_fixups: i32,                       // num of vertexFileFixup_t
    pub fixup_table_start: i32,                // offset from base to fixup table
    pub vertex_data_start: i32,                // offset from base to vertex block
    pub tangent_data_start: i32,               // offset from base to tangent block
}

// apply sequentially to lod sorted vertex and tangent pools to re-establish mesh order
#[derive(Copy, Clone)]
pub struct VVDFileFixupTable {
    pub lod: i32,              // used to skip culled root lod
    pub source_vertex_id: i32, // absolute index from start of vertex/tangent blocks
    pub num_vertexes: i32,
}

// NOTE: This is exactly 48 bytes
#[derive(Copy, Clone)]
pub struct VVDFileVertex {
    pub bone_weight: VVDFileBoneWeight,
    pub vec_position: super::SourceModelVector,
    pub vec_normal: super::SourceModelVector,
    pub vec_tex_coord: super::SourceModelVector2D,
}

// Bone weighting (0-12) [3xfloat] - Contains a maximum of 3 floating points numbers, one for each bone. The engine allows a maximum of 3 bones per vert. Older formats such as version 37 used 4 bones.
// Bone IDs (12-15) [3xbyte] - IDs of the bones the vertex is weighted to. The first bone will use the first float number for weighting.
// Bone count (15-16) [byte] - Number of bones the vertex is weighted to. This should be at least 1.
// Position (16-28) [3xfloat] - Floating points numbers for each axis (XYZ) in inches.
// Normals (28-40) [3xfloat] - Floating points numbers for vertex normals.
// Texture Co-ordinates (40-48) [2xfloat] - Floating points numbers for UV map. The value for V may need to inverted and incremented by 1 to get back the original value.

const MAX_NUM_BONES_PER_VERT: usize = 3;

// 16 bytes
#[derive(Copy, Clone)]
pub struct VVDFileBoneWeight {
    weight: [f32; MAX_NUM_BONES_PER_VERT],
    bone: [u8; MAX_NUM_BONES_PER_VERT],
    num_bones: u8,
}

#[derive(Clone)]
pub struct VVDFile {
    pub header: VVDFileHeader,
    pub fixup_table: Option<VVDFileFixupTable>,
    pub vertices: Vec<VVDFileVertex>,
}

pub fn read_vvd_file_by_name(name: &str) -> Result<VVDFile, VVDDeserializeError> {
    let path = format!("{}{}{}", super::MODEL_PATH, name, ".vvd");
    read_vvd_file_from_disk(&path)
}

/// Loads a Source Engine vvd file from disk and returns it parsed to an instance of the VVDFile struct.
///
/// # Errors
///
/// If there is any issue loading the vvd file from disk, an Err variant will
/// be returned.
pub fn read_vvd_file_from_disk(path: &str) -> Result<VVDFile, VVDDeserializeError> {
    let mut vvd_file = match File::open(path) {
        Ok(f) => f,
        Err(_e) => {
            return Err(VVDDeserializeError::new(
                "Unable to open vvd file from disk",
            ));
        }
    };

    let mut vvd_data_bytes = Vec::<u8>::new();
    match vvd_file.read_to_end(&mut vvd_data_bytes) {
        Ok(b) => b,
        Err(_e) => return Err(VVDDeserializeError::new("Error reading vvd file contents")),
    };

    let header: &VVDFileHeader = copy_c_struct!(
        VVDFileHeader,
        0,
        0,
        vvd_data_bytes
    );

    if header.id != VVD_HEADER {
        return Err(VVDDeserializeError::new(
            "vvd header not correct; expected [0x49, 0x44, 0x53, 0x56]",
        ));
    }

    if header.num_fixups > 0 {
        warn!("loading fixups -- not working yet");
        let fixup_start_index: usize = header.fixup_table_start as usize;
        let fixup_end_index: usize =
            header.fixup_table_start as usize + mem::size_of::<VVDFileFixupTable>();

        let fixup_table: &VVDFileFixupTable = copy_c_struct!(
            VVDFileFixupTable,
            fixup_start_index,
            0,
            vvd_data_bytes
        );

        assert!(fixup_end_index <= header.vertex_data_start as usize);

        // How the fixup table is used when loading vertex data:

        // If there's no fixup table (numFixups is 0) then all the vertices are loaded
        // If there is, then the engine iterates through all the fixups. If the LOD of a fixup is superior or equal to the required LOD, it loads the vertices associated with that fixup (see sourceVertexID and numVertices).
        // A fixup seems to be generated for instance if a vertex has a different position from a parent LOD.
    }

    // A list of vertices follows the header
    let mut vertices: Vec<VVDFileVertex> = Vec::new();

    let mut vertex_start_index = header.vertex_data_start as usize;
    let mut vertex_end_index = vertex_start_index + mem::size_of::<VVDFileVertex>();
    let mut i = 0 as usize;
    while vertex_start_index <= header.tangent_data_start as usize {
        let vertex: &VVDFileVertex = copy_c_struct!(
            VVDFileVertex,
            vertex_start_index,
            0,
            vvd_data_bytes
        );

        vertices.push(*vertex);

        i = i + 1;
        vertex_start_index =
            header.vertex_data_start as usize + mem::size_of::<VVDFileVertex>() * i;
        vertex_end_index = vertex_start_index + mem::size_of::<VVDFileVertex>();
    }

    Ok(VVDFile {
        header: *header,
        fixup_table: None,
        vertices: vertices,
    })
}
