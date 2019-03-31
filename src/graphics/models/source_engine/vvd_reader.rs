use std::error::Error;
use std::ffi::CStr;
use std::fmt;
use std::fs::File;
use std::io::Read;

// https://developer.valvesoftware.com/wiki/VVD

#[derive(Debug)]
pub struct VVDDeserializeError {
	details: String,
}

impl VVDDeserializeError {
	fn new(msg: &str) -> VVDDeserializeError {
		VVDDeserializeError {
			details: msg.to_string(),
		}
	}
}

impl fmt::Display for VVDDeserializeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.details)
	}
}

impl Error for VVDDeserializeError {
	fn description(&self) -> &str {
		&self.details
	}
}

// >>> import struct
// >>> struct.unpack("<i", "\x49\x44\x53\x56")
// (1448297545,)
const VVD_HEADER: i32 = 1448297545;

// https://github.com/ValveSoftware/source-sdk-2013/blob/master/sp/src/public/studio.h
const MAX_NUM_LODS: usize = 8;

// these structures can be found in <mod folder>/src/public/studio.h
#[derive(Copy, Clone)]
pub struct VVDFileHeader
{
	pub id: i32, // MODEL_VERTEX_FILE_ID
	pub version: i32, // MODEL_VERTEX_FILE_VERSION
	pub checksum: i32, // same as studiohdr_t, ensures sync
	pub num_lods: i32, // num of valid lods
	pub num_lod_vertexes: [i32; MAX_NUM_LODS], // num verts for desired root lod
	pub num_fixups: i32, // num of vertexFileFixup_t
	pub fixup_table_start: i32, // offset from base to fixup table
	pub vertex_data_start: i32, // offset from base to vertex block
	pub tangent_data_start: i32, // offset from base to tangent block
}

#[derive(Copy, Clone)]
pub struct VVDFile {
	pub header: VVDFileHeader,
}

pub fn read_vvd_file_by_name(name: &str) -> Result<VVDFile, VVDDeserializeError> {
	let path = format!("{}{}{}", super::MODEL_PATH, name, ".vvd");
	read_vvd_file_from_disk(&path)
}

/// Loads a Source Engine vvd file from disk and returns it parsed to an instance of the VVDFile struct.
///
/// # Errors
///
/// If there is any issue loading the vvd file from disk, an Err variant will
/// be returned.
pub fn read_vvd_file_from_disk(path: &str) -> Result<VVDFile, VVDDeserializeError> {
	let mut vvd_file = match File::open(path) {
		Ok(f) => f,
		Err(_e) => {
			return Err(VVDDeserializeError::new(
				"Unable to open vvd file from disk",
			));
		}
	};

	let mut vvd_data_bytes = Vec::<u8>::new();
	match vvd_file.read_to_end(&mut vvd_data_bytes) {
		Ok(b) => b,
		Err(_e) => return Err(VVDDeserializeError::new("Error reading vvd file contents")),
	};

	let data_ptr: *const u8 = vvd_data_bytes.as_ptr();
	let header_ptr: *const VVDFileHeader = data_ptr as *const _;
	let header: &VVDFileHeader = unsafe { &*header_ptr };

	if header.id != VVD_HEADER {
		return Err(VVDDeserializeError::new(
			"vvd header not correct; expected [0x49, 0x44, 0x53, 0x56]",
		));
	}

	Ok(VVDFile {
		header: *header,
	})
}
