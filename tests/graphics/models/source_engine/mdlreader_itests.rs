extern crate rust_tower_defense;

use std::mem;

use rust_tower_defense::graphics::models::source_engine::mdl_reader;

#[test]
fn load_model() {
    let model =
        mdl_reader::read_mdl_file_from_disk("source_assets/models/player/ctm_sas_variantA.mdl")
            .unwrap();

    // The header of an MDL file should be "IDST"
    let mdl_header: [u8; 4] = [0x49, 0x44, 0x53, 0x54];
    let mdl_header_i32: i32 = unsafe { mem::transmute::<[u8; 4], i32>(mdl_header) };

    assert_eq!(model.header.id, mdl_header_i32);
    assert_eq!(model.header.version, 49);
    assert_eq!(model.header.data_length, 91108);
    assert_eq!(model.header.bodypart_count, 1);

    // the wiki docs say this should be 408, but the header they included is only 400
    assert_eq!(mem::size_of::<mdl_reader::MDLFileHeader>(), 400);

    assert_eq!(model.name, "player/ctm_sas_variantA.mdl");
}

#[test]
#[should_panic]
fn load_invalid_model() {
    mdl_reader::read_mdl_file_from_disk("source_assets/models/invalid.mdl").unwrap();
}
