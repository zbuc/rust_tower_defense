use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::mem;

use crate::copy_c_struct;

// https://developer.valvesoftware.com/wiki/Model

#[derive(Debug)]
pub struct VTXDeserializeError {
    details: String,
}

impl VTXDeserializeError {
    fn new(msg: &str) -> VTXDeserializeError {
        VTXDeserializeError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for VTXDeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for VTXDeserializeError {
    fn description(&self) -> &str {
        &self.details
    }
}

const OPTIMIZED_MODEL_FILE_VERSION: i32 = 7;

/// https://developer.valvesoftware.com/wiki/VTX
#[repr(C)]
#[derive(Copy, Clone)]
pub struct VTXFileHeader {
    // file version as defined by OPTIMIZED_MODEL_FILE_VERSION (currently 7)
    pub version: i32,

    // hardware params that affect how the model is to be optimized.
    vert_cache_size: i32,
    max_bones_per_strip: u16,
    max_bones_per_tri: u16,
    max_bones_per_vert: i32,

    // must match checkSum in the .mdl
    pub checksum: i32,

    num_lods: i32, // Also specified in ModelHeader_t's and should match

    // Offset to materialReplacementList Array. one of these for each LOD, 8 in total
    material_replacement_list_offset: i32,

    //Defines the size and location of the body part array
    pub num_body_parts: i32,
    pub body_part_offset: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VTXFileBodyPartHeader {
    //Model array
    pub num_models: i32,
    pub model_offset: i32,
}

// This maps one to one with models in the mdl file.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct VTXFileModelHeader {
    // LOD mesh array
    pub num_lods: i32, // This is also specified in FileHeader_t
    pub lod_offset: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VTXFileMeshHeader {
    pub num_strip_groups: i32,
    pub strip_group_header_offset: i32,

    pub flags: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VTXFileModelLODHeader {
    pub num_meshes: i32,
    pub mesh_offset: i32,

    pub switch_point: f32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VTXFileStripGroupHeader {
    // These are the arrays of all verts and indices for this mesh.  strips index into this.
    pub num_verts: i32,
    pub vert_offset: i32,

    pub num_indices: i32,
    pub index_offset: i32,

    pub num_strips: i32,
    pub strip_offset: i32,

    pub flags: u8,
}

#[derive(Clone)]
pub struct StripGroup {
    pub header: VTXFileStripGroupHeader,
}

#[derive(Clone)]
pub struct Mesh {
    pub header: VTXFileMeshHeader,
    pub strip_groups: Vec<StripGroup>,
}

#[derive(Clone)]
pub struct LOD {
    pub header: VTXFileModelLODHeader,
    pub meshes: Vec<Mesh>,
}

#[derive(Clone)]
pub struct Model {
    pub header: VTXFileModelHeader,
    pub lods: Vec<LOD>,
}

#[derive(Clone)]
pub struct BodyPart {
    pub header: VTXFileBodyPartHeader,
    pub models: Vec<Model>,
}

#[derive(Clone)]
pub struct VTXFile {
    pub header: VTXFileHeader,
    pub bodyparts: Vec<BodyPart>,
}

struct VTXDeserializer {
    path: String,
}

impl VTXDeserializer {
    pub fn new(path: String) -> Self {
        VTXDeserializer { path }
    }

    pub fn read_lods(
        &self,
        model_header: &VTXFileModelHeader,
        model_start_index: usize,
        body_part_num: usize,
        model_num: usize,
        vtx_data_bytes: &[u8],
    ) -> Result<Vec<LOD>, VTXDeserializeError> {
        let mut lods: Vec<LOD> = Vec::new();

        info!("num lods {}", model_header.num_lods);
        for lod_num in 0..model_header.num_lods {
            debug!(
                "Loading body part {}, model {}, lod {}",
                body_part_num, model_num, lod_num
            );
            let lod_start_index = model_start_index
                + model_header.lod_offset as usize
                + (lod_num as usize * mem::size_of::<VTXFileModelLODHeader>()) as usize;
            let lod_header: &VTXFileModelLODHeader = copy_c_struct!(
                VTXFileModelLODHeader,
                lod_start_index,
                lod_num,
                vtx_data_bytes
            );

            let mut meshes: Vec<Mesh> = Vec::new();

            info!("num meshes {}", lod_header.num_meshes);
            for mesh_num in 0..lod_header.num_meshes {
                info!(
                    "Loading body part {}, model {}, lod {}, mesh {}",
                    body_part_num, model_num, lod_num, mesh_num
                );
                let mesh_start_index = lod_start_index
                    + lod_header.mesh_offset as usize
                    + (mesh_num as usize * mem::size_of::<VTXFileMeshHeader>()) as usize;
                info!("mesh_start_index: {}", mesh_start_index);
                // panic!("eff");
                let mesh_header: &VTXFileMeshHeader = copy_c_struct!(
                    VTXFileMeshHeader,
                    mesh_start_index,
                    mesh_num,
                    vtx_data_bytes
                );

                let mut strip_groups: Vec<StripGroup> = Vec::new();

                for strip_group_num in 0..mesh_header.num_strip_groups {
                    // info!(
                    //     "Loading body part {}, model {}, lod {}, mesh {}, strip group {}",
                    //     body_part_num, model_num, lod_num, mesh_num, strip_group_num
                    // );

                    // let strip_group_start_index = mesh_start_index
                    //     + mesh_header.strip_group_header_offset as usize
                    //     + (strip_group_num as usize
                    //         * mem::size_of::<VTXFileStripGroupHeader>())
                    //         as usize;
                    // let strip_header: &VTXFileStripGroupHeader = copy_c_struct!(
                    //     VTXFileStripGroupHeader,
                    //     strip_group_start_index,
                    //     strip_group_num,
                    //     vtx_data_bytes
                    // );

                    // strip_groups.push(StripGroup {
                    //     header: *strip_header,
                    // });
                }

                meshes.push(Mesh {
                    header: *mesh_header,
                    strip_groups,
                });
            }

            lods.push(LOD {
                header: *lod_header,
                meshes,
            });
        }

        Ok(lods)
    }

    pub fn read_models(
        &self,
        bodyparts_header: &VTXFileBodyPartHeader,
        body_part_num: i32,
        bodypart_start_index: usize,
        vtx_data_bytes: &[u8],
    ) -> Result<Vec<Model>, VTXDeserializeError> {
        let mut model_headers: Vec<VTXFileModelHeader> = Vec::new();
        let mut model_start_indices: Vec<usize> = Vec::new();

        for model_num in 0..bodyparts_header.num_models {
            debug!("Loading body part {}, model {}", body_part_num, model_num);
            let model_start_index = bodypart_start_index
                + bodyparts_header.model_offset as usize
                + (model_num as usize * mem::size_of::<VTXFileBodyPartHeader>() as usize);
            let model_header: &VTXFileModelHeader = copy_c_struct!(
                VTXFileModelHeader,
                bodypart_start_index + bodyparts_header.model_offset as usize,
                model_num,
                vtx_data_bytes
            );
            model_headers.push(*model_header);
            model_start_indices.push(model_start_index);
        }

        let mut models: Vec<Model> = Vec::new();

        for model_num in 0..bodyparts_header.num_models {
            let model_header = model_headers[model_num as usize];
            let model_start_index = model_start_indices[model_num as usize];

            let mut lods: Vec<LOD> = self.read_lods(
                &model_header,
                model_start_index,
                body_part_num as usize,
                model_num as usize,
                vtx_data_bytes,
            )?;

            models.push(Model {
                header: model_header,
                lods,
            });
        }

        Ok(models)
    }

    pub fn read_bodyparts(
        &self,
        file_header: &VTXFileHeader,
        vtx_data_bytes: &[u8],
    ) -> Result<Vec<BodyPart>, VTXDeserializeError> {
        let mut bodyparts_headers: Vec<VTXFileBodyPartHeader> = Vec::new();
        let mut bodyparts_start_indices: Vec<usize> = Vec::new();

        // First, load the headers because they are all stored consecutively
        for body_part_num in 0..file_header.num_body_parts {
            debug!("Loading body part {}", body_part_num);
            let bodypart_start_index = file_header.body_part_offset as usize
                + body_part_num as usize * mem::size_of::<VTXFileBodyPartHeader>();
            let bodyparts_header: &VTXFileBodyPartHeader = copy_c_struct!(
                VTXFileBodyPartHeader,
                bodypart_start_index,
                body_part_num,
                vtx_data_bytes
            );

            bodyparts_headers.push(*bodyparts_header);
            bodyparts_start_indices.push(bodypart_start_index);
        }

        let mut bodyparts: Vec<BodyPart> = Vec::new();

        for body_part_num in 0..file_header.num_body_parts {
            let bodyparts_header = bodyparts_headers[body_part_num as usize];
            let bodypart_start_index = bodyparts_start_indices[body_part_num as usize];

            // Then, load the models
            let models: Vec<Model> = self.read_models(
                &bodyparts_header,
                body_part_num,
                bodypart_start_index,
                vtx_data_bytes,
            )?;

            bodyparts.push(BodyPart {
                header: bodyparts_headers[body_part_num as usize],
                models,
            });
        }

        Ok(bodyparts)
    }

    pub fn deserialize(&mut self) -> Result<VTXFile, VTXDeserializeError> {
        let mut vtx_file = match File::open(&self.path) {
            Ok(f) => f,
            Err(_e) => {
                return Err(VTXDeserializeError::new(
                    "Unable to open vtx file from disk",
                ));
            }
        };

        let mut vtx_data_bytes = Vec::<u8>::new();
        match vtx_file.read_to_end(&mut vtx_data_bytes) {
            Ok(b) => b,
            Err(_e) => return Err(VTXDeserializeError::new("Error reading vtx file contents")),
        };

        let header: &VTXFileHeader = copy_c_struct!(VTXFileHeader, 0, 0, vtx_data_bytes);

        // The first 4 bytes of a VTX file should be a version, 7 (OPTIMIZED_MODEL_FILE_VERSION)
        if header.version != OPTIMIZED_MODEL_FILE_VERSION {
            return Err(VTXDeserializeError::new(
                "VTX version not correct; expected 7",
            ));
        }

        // let mut bodyparts: Vec<BodyPart> = Vec::new();
        let mut bodyparts: Vec<BodyPart> = self.read_bodyparts(header, &vtx_data_bytes)?;

        // XXX there *really* should be actual checked deserialization here because this will produce unexpected behavior
        // for improperly formatted models -- but I'm *personally* only ever going to feed it good models ;)

        Ok(VTXFile {
            header: *header,
            bodyparts,
        })
    }
}

/// Loads a Source Engine vtx file from disk and returns it parsed to an instance of the VTXFile struct.
/// https://github.com/ValveSoftware/source-sdk-2013/blob/master/sp/src/public/optimize.h
///
/// # Errors
///
/// If there is any issue loading the VTX file from disk, an Err variant will
/// be returned.
pub fn read_vtx_file_from_disk(path: &str) -> Result<VTXFile, VTXDeserializeError> {
    let mut deserializer = VTXDeserializer::new(path.to_string());

    deserializer.deserialize()
}

pub fn read_vtx_file_by_name(name: &str) -> Result<VTXFile, VTXDeserializeError> {
    let path = format!("{}{}{}", super::MODEL_PATH, name, ".dx90.vtx");
    read_vtx_file_from_disk(&path)
}
