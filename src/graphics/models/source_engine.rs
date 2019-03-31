pub mod mdl_reader;
pub mod vtx_reader;

use std::error::Error;

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
}

pub fn read_source_engine_model(name: &str) -> Result<SourceEngineModel, Box<dyn Error>> {
    let mdl_file = mdl_reader::read_mdl_file_by_name(name)?;
    let vtx_file = vtx_reader::read_vtx_file_by_name(name)?;

    Ok(SourceEngineModel {
        mdl_file,
        vtx_file,
    })
}