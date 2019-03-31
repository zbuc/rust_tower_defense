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
pub struct SourceEngineModel {
    pub mdl_file: mdl_reader::MDLFile,
    pub vtx_file: vtx_reader::VTXFile,
    pub vvd_file: vvd_reader::VVDFile,
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

    Ok(SourceEngineModel {
        mdl_file,
        vtx_file,
        vvd_file,
    })
}
