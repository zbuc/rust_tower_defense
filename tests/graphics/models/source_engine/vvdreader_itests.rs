extern crate rust_tower_defense;

use std::mem;

use rust_tower_defense::graphics::models::source_engine::vvd_reader;

#[test]
fn load_vvd() {
    let vvdfile =
        vvd_reader::read_vvd_file_from_disk("source_assets/models/player/ctm_sas_variantA.vvd")
            .unwrap();

    // The header of a VVD file should be "IDSV"
    let vvd_header: [u8; 4] = [0x49, 0x44, 0x53, 0x56];
    let vvd_header_i32: i32 = unsafe { mem::transmute::<[u8; 4], i32>(vvd_header) };

    assert_eq!(vvdfile.header.id, vvd_header_i32);
    assert_eq!(vvdfile.header.version, 4);
    assert_eq!(
        mem::size_of::<vvd_reader::VVDFileHeader>() as i32,
        vvdfile.header.fixup_table_start
    );

    assert_eq!(mem::size_of::<vvd_reader::VVDFileFixupTable>() as i32, 12);

    assert!(vvdfile.fixup_table.is_none());

    assert_eq!(vvdfile.header.vertex_data_start, 64);
    assert_eq!(vvdfile.header.num_lods, 1);
    assert_eq!(vvdfile.header.num_lod_vertexes.len(), 8);
    assert_eq!(vvdfile.header.num_fixups, 0);

    assert_eq!(vvdfile.header.tangent_data_start, 537856);

    assert_eq!(vvdfile.vertices.len(), 11205);
}

#[test]
#[should_panic]
fn load_invalid_vvd() {
    vvd_reader::read_vvd_file_from_disk("source_assets/models/invalid.vvd").unwrap();
}
