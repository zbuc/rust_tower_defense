extern crate rust_tower_defense;

use std::mem;

use rust_tower_defense::graphics::models::source_engine::vtx_reader;

#[test]
fn load_vtx() {
    let vtxfile = vtx_reader::read_vtx_file_from_disk(
        "source_assets/models/player/ctm_sas_variantA.dx90.vtx",
    )
    .unwrap();

    // The first 4 bytes of a VTX file should be a version, 7 (OPTIMIZED_MODEL_FILE_VERSION)
    assert_eq!(vtxfile.header.version, 7);
    assert_eq!(vtxfile.header.num_body_parts, 1);
    assert_eq!(vtxfile.header.body_part_offset, 36);
    assert_eq!(
        mem::size_of::<vtx_reader::VTXFileHeader>() as i32,
        vtxfile.header.body_part_offset
    );

    assert_eq!(vtxfile.bodyparts.len(), 1);
    assert_eq!(vtxfile.bodyparts[0].header.num_models, 1);
    assert_eq!(vtxfile.bodyparts[0].header.model_offset, 8);
    
    assert_eq!(vtxfile.bodyparts[0].models.len(), 1);
    assert_eq!(vtxfile.bodyparts[0].models[0].lods.len(), 1);
}

#[test]
#[should_panic]
fn load_invalid_vtx() {
    vtx_reader::read_vtx_file_from_disk("source_assets/models/invalid.vtx").unwrap();
}
