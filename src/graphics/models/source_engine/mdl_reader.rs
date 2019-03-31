use std::error::Error;
use std::fmt;
use std::fs::{File};
use std::io::Read;
use std::ffi::{CString, CStr};

// https://developer.valvesoftware.com/wiki/Model

#[derive(Debug)]
pub struct MDLDeserializeError {
    details: String
}

impl MDLDeserializeError {
    fn new(msg: &str) -> MDLDeserializeError {
        MDLDeserializeError{details: msg.to_string()}
    }
}

impl fmt::Display for MDLDeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.details)
    }
}

impl Error for MDLDeserializeError {
    fn description(&self) -> &str {
        &self.details
    }
}

// >>> import struct
// >>> struct.unpack("<i", "\x49\x44\x53\x54")
// (1414743113,)
const MDL_HEADER: i32 = 1414743113;

#[repr(C)]
#[derive(Copy, Clone)]
struct MDLVector(f32, f32, f32);

/// https://developer.valvesoftware.com/wiki/MDL
#[repr(C)]
#[derive(Copy, Clone)]
pub struct MDLFileHeader {
    pub id: i32,		// Model format ID, such as "IDST" (0x49 0x44 0x53 0x54)
	version: i32,	// Format version number, such as 48 (0x30,0x00,0x00,0x00)
	checksum: i32,	// This has to be the same in the phy and vtx files to load!
	pub name: [u8; 64],		// The internal name of the model, padding with null bytes.
					// Typically "my_model.mdl" will have an internal name of "my_model"
	data_length: i32,	// Data size of MDL file in bytes.
 
	// A vector is 12 bytes, three 4-byte float-values in a row.
	eyeposition: MDLVector,	// Position of player viewpoint relative to model origin
	llumposition: MDLVector,	// ?? Presumably the point used for lighting when per-vertex lighting is not enabled.
	ull_min: MDLVector,	// Corner of model hull box with the least X/Y/Z values
	ull_max: MDLVector,	// Opposite corner of model hull box
    view_bbmin: MDLVector,
	view_bbmax: MDLVector,
 
	flags: i32,		// Binary flags in little-endian order. 
					// ex (00000001,00000000,00000000,11000000) means flags for position 0, 30, and 31 are set. 
					// Set model flags section for more information
 
	/*
	 * After this point, the header contains many references to offsets
	 * within the MDL file and the number of items at those offsets.
	 *
	 * Offsets are from the very beginning of the file.
	 * 
	 * Note that indexes/counts are not always paired and ordered consistently.
	 */	
 
	// mstudiobone_t
	bone_count: i32,	// Number of data sections (of type mstudiobone_t)
	bone_offset: i32,	// Offset of first data section
 
	// mstudiobonecontroller_t
	bonecontroller_count: i32,
	bonecontroller_offset: i32,
 
	// mstudiohitboxset_t
	hitbox_count: i32,
	hitbox_offset: i32,
 
	// mstudioanimdesc_t
	localanim_count: i32,
	localanim_offset: i32,
 
	// mstudioseqdesc_t
	localseq_count: i32,
	localseq_offset: i32,
 
	activitylistversion: i32, // ??
	eventsindexed: i32,	// ??
 
	// VMT texture filenames
	// mstudiotexture_t
	texture_count: i32,
	texture_offset: i32,
 
	// This offset points to a series of ints.
        // Each int value, in turn, is an offset relative to the start of this header/the-file,
        // At which there is a null-terminated string.
	texturedir_count: i32,
	texturedir_offset: i32,
 
	// Each skin-family assigns a texture-id to a skin location
	skinreference_count: i32,
	skinrfamily_count: i32,
	skinreference_index: i32,
 
	// mstudiobodyparts_t
	bodypart_count: i32,
	bodypart_offset: i32,
 
        // Local attachment points		
	// mstudioattachment_t
	attachment_count: i32,
	attachment_offset: i32,
 
	// Node values appear to be single bytes, while their names are null-terminated strings.
	localnode_count: i32,
	pub localnode_index: i32,
	localnode_name_index: i32,
 
	// mstudioflexdesc_t
	flexdesc_count: i32,
	flexdesc_index: i32,
 
	// mstudioflexcontroller_t
	flexcontroller_count: i32,
	flexcontroller_index: i32,
 
	// mstudioflexrule_t
	flexrules_count: i32,
	flexrules_index: i32,
 
	// IK probably referse to inverse kinematics
	// mstudioikchain_t
	ikchain_count: i32,
	ikchain_index: i32,
 
	// Information about any "mouth" on the model for speech animation
	// More than one sounds pretty creepy.
	// mstudiomouth_t
	mouths_count: i32,
	mouths_index: i32,
 
	// mstudioposeparamdesc_t
	localposeparam_count: i32,
	localposeparam_index: i32,
 
	/*
	 * For anyone trying to follow along, as of this writing,
	 * the next "surfaceprop_index" value is at position 0x0134 (308)
	 * from the start of the file.
	 */
 
	// Surface property value (single null-terminated string)
	surfaceprop_index: i32,
 
	// Unusual: In this one index comes first, then count.
	// Key-value data is a series of strings. If you can't find
	// what you're interested in, check the associated PHY file as well.
	keyvalue_index: i32,
	keyvalue_count: i32,
 
	// More inverse-kinematics
	// mstudioiklock_t
	iklock_count: i32,
	iklock_index: i32,
 
 
	mass: f32, 		// Mass of object (4-bytes)
	contents: i32,	// ??
 
	// Other models can be referenced for re-used sequences and animations
	// (See also: The $includemodel QC option.)
	// mstudiomodelgroup_t
	includemodel_count: i32,
	includemodel_index: i32,
	
	virtual_model: i32,	// Placeholder for mutable-void*
 
	// mstudioanimblock_t
	animblocks_name_index: i32,
	animblocks_count: i32,
	animblocks_index: i32,
	
	animblock_model: i32, // Placeholder for mutable-void*

	// Points to a series of bytes?
	bonetablename_index: i32,
	
	vertex_base: i32,	// Placeholder for void*
	offset_base: i32,	// Placeholder for void*
	
	// Used with $constantdirectionallight from the QC 
	// Model should have flag #13 set if enabled
	directionaldotproduct: u8,
	
	root_lod: u8,	// Preferred rather than clamped
	
	// 0 means any allowed, N means Lod 0 -> (N-1)
	num_allowed_root_lods: u8,
	
	unused1: u8, // ??
	unused2: i32, // ??
	
	// mstudioflexcontrollerui_t
	flexcontrollerui_count: i32,
	flexcontrollerui_index: i32,
	
	/**
	 * Offset for additional header information.
	 * May be zero if not present, or also 408 if it immediately 
	 * follows this studiohdr_t
	 */
	// studiohdr2_t
	studiohdr2index: i32,

	unused3: i32, // ??
	
	// As of this writing, the header is 408 bytes long in total -- OR THAT'S WHAT VALVE SAYS.
	// I tried using the header directly from C and received 400 bytes.
}

#[derive(Clone)]
pub struct MDLFile {
    pub header: MDLFileHeader,
	pub name: String,
}

/// Loads a Source Engine mdl file from disk and returns it parsed to an instance of the MDLFile struct.
/// I pieced this together from publicly available documentation, e.g. https://developer.valvesoftware.com/wiki/MDL
/// and reverse engineering the mdllib.dll included with Source SDK 2013 and used in the example hlmv.exe model viewer.
///
/// # Errors
///
/// If there is any issue loading the mdl file from disk, an Err variant will
/// be returned.
pub fn read_mdl_file_from_disk(path: &str) -> Result<MDLFile, MDLDeserializeError> {
    let mut model_file = match File::open(path) {
        Ok(f) => f,
        Err(_e) => return Err(MDLDeserializeError::new("Unable to open mdl file from disk")),
    };

    let mut model_data_bytes = Vec::<u8>::new();
    match model_file.read_to_end(&mut model_data_bytes) {
        Ok(b) => b,
        Err(_e) => return Err(MDLDeserializeError::new("Error reading mdl file contents")),
    };

    let data_ptr: *const u8 = model_data_bytes.as_ptr();
    let header_ptr: *const MDLFileHeader = data_ptr as *const _;
    let header: &MDLFileHeader = unsafe { &*header_ptr };

    if header.id != MDL_HEADER {
        return Err(MDLDeserializeError::new("mdl header not correct; expected [0x49, 0x44, 0x53, 0x54]"));
    }

	// from_bytes_with_nul requires no internal NUL chars, so we need to find
	// the first index of a NUL char in the string
	let first_null = &header.name.iter().position(|&r| r == 0x0).unwrap() + 1;
	let name = CStr::from_bytes_with_nul(&header.name[0..first_null]).expect("CStr::from_bytes_with_nul failed");
	let name = match name.to_str() {
		Ok(s) => s,
		Err(_e) => return Err(MDLDeserializeError::new("Error converting name to string")),
	};

    // XXX there *really* should be actual checked deserialization here because this will produce unexpected behavior
    // for improperly formatted models -- but I'm *personally* only ever going to feed it good models ;)

    if header.studiohdr2index == 408 {
        // additional header directly follows this header
        info!("studiohdr2index exists directly following main header");
    } else if header.studiohdr2index == 0 {
        // no additional header
        info!("no additional header");
    } else {
        info!("additional header at {}", header.studiohdr2index);
    }

    Ok(MDLFile{
        header: *header,
		name: String::from(name),
    })
}