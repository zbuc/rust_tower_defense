extern crate rust_tower_defense;

use std::mem;

use rust_tower_defense::graphics::mdl_reader;

#[test]
fn load_model() {
    let model = mdl_reader::read_model_file_from_disk("source_assets/models/ctm_sas_varianta.mdl").unwrap();

    // The header of an MDL file should be "IDST"
    let mdl_header: [u8; 4] = [0x49, 0x44, 0x53, 0x54];
    let mdl_header_i32: i32 = unsafe {
         mem::transmute::<[u8; 4], i32>(mdl_header)
    };

    assert_eq!(model.header.id, mdl_header_i32);
}

#[test]
#[should_panic]
fn load_invalid_model() {
    mdl_reader::read_model_file_from_disk("source_assets/models/invalid_model.mdl").unwrap();
}