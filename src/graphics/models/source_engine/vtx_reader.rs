use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::mem;

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

#[derive(Copy, Clone)]
pub struct VTXFileBodyPartHeader {
    //Model array
    pub num_models: i32,
    pub model_offset: i32,
}

// This maps one to one with models in the mdl file.
#[derive(Copy, Clone)]
pub struct VTXFileModelHeader {
    // LOD mesh array
    pub num_lods: i32, // This is also specified in FileHeader_t
    pub lod_offset: i32,
}

#[derive(Clone)]
pub struct Model {
    pub header: VTXFileModelHeader,
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

pub fn read_vtx_file_by_name(name: &str) -> Result<VTXFile, VTXDeserializeError> {
    let path = format!("{}{}{}", super::MODEL_PATH, name, ".dx90.vtx");
    read_vtx_file_from_disk(&path)
}

/// Loads a Source Engine vtx file from disk and returns it parsed to an instance of the VTXFile struct.
/// https://github.com/ValveSoftware/source-sdk-2013/blob/master/sp/src/public/optimize.h
///
/// # Errors
///
/// If there is any issue loading the VTX file from disk, an Err variant will
/// be returned.
pub fn read_vtx_file_from_disk(path: &str) -> Result<VTXFile, VTXDeserializeError> {
    let mut vtx_file = match File::open(path) {
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

    let header_data_ptr: *const u8 = vtx_data_bytes[0..mem::size_of::<VTXFileHeader>()].as_ptr();
    let header_ptr: *const VTXFileHeader = header_data_ptr as *const _;
    let header: &VTXFileHeader = unsafe { &*header_ptr };

    // The first 4 bytes of a VTX file should be a version, 7 (OPTIMIZED_MODEL_FILE_VERSION)
    if header.version != OPTIMIZED_MODEL_FILE_VERSION {
        return Err(VTXDeserializeError::new(
            "VTX version not correct; expected 7",
        ));
    }

    let mut bodyparts: Vec<BodyPart> = Vec::new();
    let mut bodypart_headers: Vec<VTXFileBodyPartHeader> = Vec::new();

    for x in 0..header.num_body_parts {
        debug!("Loading body part {}", x);
        let bodypart_start_index = (x + 1) as usize * mem::size_of::<VTXFileHeader>();
        let bodypart_end_index = bodypart_start_index + mem::size_of::<VTXFileBodyPartHeader>();

        let bodyparts_data_ptr: *const u8 =
            vtx_data_bytes[bodypart_start_index..bodypart_end_index].as_ptr();
        let bodyparts_ptr: *const VTXFileBodyPartHeader = bodyparts_data_ptr as *const _;
        let bodyparts_header: &VTXFileBodyPartHeader = unsafe { &*bodyparts_ptr };

        let mut models: Vec<Model> = Vec::new();

        for y in 0..bodyparts_header.num_models {
            let model_start_index =
                bodypart_start_index + ((y + 1) * bodyparts_header.model_offset) as usize;
            let model_end_index = model_start_index + mem::size_of::<VTXFileModelHeader>();

            let model_data_ptr: *const u8 =
                vtx_data_bytes[model_start_index..model_end_index].as_ptr();
            let model_ptr: *const VTXFileModelHeader = model_data_ptr as *const _;
            let model_header: &VTXFileModelHeader = unsafe { &*model_ptr };

            models.push(Model {
                header: *model_header,
            });
        }

        bodyparts.push(BodyPart {
            header: *bodyparts_header,
            models: models,
        });
    }

    // XXX there *really* should be actual checked deserialization here because this will produce unexpected behavior
    // for improperly formatted models -- but I'm *personally* only ever going to feed it good models ;)

    Ok(VTXFile {
        header: *header,
        bodyparts: bodyparts,
    })
}
