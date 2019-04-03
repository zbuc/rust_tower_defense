pub mod mdl_reader;
pub mod vtx_reader;
pub mod vvd_reader;

use std::error::Error;
use std::fmt;

pub const MODEL_PATH: &str = "source_assets/models/";

/// SourceEngineModel represents a Source Engine 2013 player model
/// and is designed in a way to make an easy interface with the rest
/// of the game.
///
/// Might want to remove the references to the files here and make more
/// properties.
#[derive(Clone)]
pub struct SourceEngineModel {
    pub mdl_file: mdl_reader::MDLFile,
    pub vtx_file: vtx_reader::VTXFile,
    pub vvd_file: vvd_reader::VVDFile,
    pub vertices: Vec<super::super::Vertex>,
    pub normals: Vec<super::super::Vertex>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SourceModelVector(f32, f32, f32);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SourceModelVector2D(f32, f32);

// @chris see here: https://stackoverflow.com/a/25411013/3317191
// tl;dr instantiate the struct you want to fill with zeroed or uninitialised memory, then unsafely access its memory as a mutable slice and read into that. (edited) 
// Or just implement a parser for the serialised struct using byteorder and call it a day.
// you can also first read the data into any kind of `AsRef<[u8]>` container and then use `str::from_utf8` to obtain a `str` of its contents
// or if you read into a `Vec` you can do a zero copy conversion to a `String` with `String::from_utf8()`
#[macro_export]
macro_rules! copy_c_struct {
    ($type:ty,$start_index:expr,$i:ident,$data_ptr:ident) => {{
        let struct_start_index = mem::size_of::<$type>() * $i as usize + $start_index;
        let struct_end_index = struct_start_index + mem::size_of::<$type>();

        let struct_data_ptr: *const u8 = $data_ptr[struct_start_index..struct_end_index].as_ptr();
        let struct_ptr: *const $type = struct_data_ptr as *const _;
        let struct_from_c: &$type = unsafe { &*struct_ptr };

        struct_from_c
    }};
    ($type:ty,$start_index:expr,$i:expr,$data_ptr:ident) => {{
        let struct_start_index = mem::size_of::<$type>() * $i as usize + $start_index;
        let struct_end_index = struct_start_index + mem::size_of::<$type>();

        let struct_data_ptr: *const u8 = $data_ptr[struct_start_index..struct_end_index].as_ptr();
        let struct_ptr: *const $type = struct_data_ptr as *const _;
        let struct_from_c: &$type = unsafe { &*struct_ptr };

        struct_from_c
    }};
}

#[derive(Debug)]
pub struct ModelLoadError {
    details: String,
}

impl ModelLoadError {
    fn new(msg: &str) -> ModelLoadError {
        ModelLoadError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for ModelLoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for ModelLoadError {
    fn description(&self) -> &str {
        &self.details
    }
}

pub fn read_source_engine_model(name: &str) -> Result<SourceEngineModel, Box<dyn Error>> {
    let mdl_file = mdl_reader::read_mdl_file_by_name(name)?;
    let vtx_file = vtx_reader::read_vtx_file_by_name(name)?;
    let vvd_file = vvd_reader::read_vvd_file_by_name(name)?;

    if mdl_file.header.bodypart_count as usize != vtx_file.bodyparts.len() {
        return Err(Box::new(ModelLoadError::new(&format!(
            "Unable to load Source engine model {}",
            name
        ))));
    }

    if mdl_file.header.checksum != vvd_file.header.checksum
        || vvd_file.header.checksum != vtx_file.header.checksum
    {
        return Err(Box::new(ModelLoadError::new("Model checksum mismatch")));
    }

    let mut vertices: Vec<super::super::Vertex> = Vec::new();
    for vertex in vvd_file.vertices.iter() {
        vertices.push(super::super::Vertex {
            position: (
                vertex.vec_position.0,
                vertex.vec_position.1,
                vertex.vec_position.2,
            ),
        });
    }

    let mut normals: Vec<super::super::Vertex> = Vec::new();
    for vertex in vvd_file.vertices.iter() {
        normals.push(super::super::Vertex {
            position: (
                vertex.vec_normal.0,
                vertex.vec_normal.1,
                vertex.vec_normal.2,
            ),
        });
    }

    Ok(SourceEngineModel {
        mdl_file,
        vtx_file,
        vvd_file,
        vertices,
        normals,
    })
}
