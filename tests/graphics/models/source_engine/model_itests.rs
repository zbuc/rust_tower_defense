extern crate rust_tower_defense;

use std::mem;

use rust_tower_defense::graphics::models::source_engine;

#[test]
fn load_model() {
    let model = source_engine::read_source_engine_model("player/ctm_sas_variantA").unwrap();

    assert_eq!(model.mdl_file.name, "player/ctm_sas_variantA.mdl");

    assert_eq!(
        model.mdl_file.header.checksum,
        model.vvd_file.header.checksum
    );
    assert_eq!(
        model.vvd_file.header.checksum,
        model.vtx_file.header.checksum
    );
}

#[test]
#[should_panic]
fn load_invalid_model() {
    let model = source_engine::read_source_engine_model("invalid").unwrap();
}
