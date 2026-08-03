use super::{ColorMode, Mii};
use crate::{Error, MiiFormat, Result};

const RFL_FACE_LINE_AND_MAKE: [[u8; 2]; 12] = [
	[0, 0],
	[0, 1],
	[0, 6],
	[0, 9],
	[5, 0],
	[2, 0],
	[3, 0],
	[7, 0],
	[8, 0],
	[0, 10],
	[9, 0],
	[11, 0],
];

pub(super) fn decode_studio_url(bytes: &[u8]) -> Result<[u8; 46]> {
	let encoded: &[u8; 47] = bytes
		.try_into()
		.map_err(|_| Error::UnsupportedDataLength(bytes.len()))?;
	let mut decoded = [0_u8; 46];
	let mut previous = encoded[0];
	for (output, &input) in decoded.iter_mut().zip(&encoded[1..]) {
		*output = input.wrapping_sub(7) ^ previous;
		previous = input;
	}
	Ok(decoded)
}

pub(super) fn decode_switch_char_info(bytes: &[u8]) -> Result<Mii> {
	let bytes: &[u8; 88] = bytes
		.try_into()
		.map_err(|_| Error::UnsupportedDataLength(bytes.len()))?;
	let mut mii = Mii::empty(ColorMode::Common);
	mii.favorite_color = bytes[39];
	mii.gender = bytes[40];
	mii.height = bytes[41];
	mii.build = bytes[42];
	mii.faceline_type = bytes[45];
	mii.faceline_color = bytes[46];
	mii.faceline_wrinkle = bytes[47];
	mii.faceline_make = bytes[48];
	mii.hair_type = bytes[49];
	mii.hair_color = bytes[50];
	mii.hair_flip = bytes[51];
	mii.eye_type = bytes[52];
	mii.eye_color = bytes[53];
	mii.eye_scale = bytes[54];
	mii.eye_aspect = bytes[55];
	mii.eye_rotate = bytes[56];
	mii.eye_x = bytes[57];
	mii.eye_y = bytes[58];
	mii.eyebrow_type = bytes[59];
	mii.eyebrow_color = bytes[60];
	mii.eyebrow_scale = bytes[61];
	mii.eyebrow_aspect = bytes[62];
	mii.eyebrow_rotate = bytes[63];
	mii.eyebrow_x = bytes[64];
	mii.eyebrow_y = bytes[65];
	mii.nose_type = bytes[66];
	mii.nose_scale = bytes[67];
	mii.nose_y = bytes[68];
	mii.mouth_type = bytes[69];
	mii.mouth_color = bytes[70];
	mii.mouth_scale = bytes[71];
	mii.mouth_aspect = bytes[72];
	mii.mouth_y = bytes[73];
	mii.beard_color = bytes[74];
	mii.beard_type = bytes[75];
	mii.mustache_type = bytes[76];
	mii.mustache_scale = bytes[77];
	mii.mustache_y = bytes[78];
	mii.glass_type = bytes[79];
	mii.glass_color = bytes[80];
	mii.glass_scale = bytes[81];
	mii.glass_y = bytes[82];
	mii.mole_type = bytes[83];
	mii.mole_scale = bytes[84];
	mii.mole_x = bytes[85];
	mii.mole_y = bytes[86];
	Ok(mii)
}

pub(super) const fn decode_switch_core_data(bytes: &[u8]) -> Result<Mii> {
	if bytes.len() != 48 && bytes.len() != 68 {
		return Err(Error::UnsupportedDataLength(bytes.len()));
	}
	let mut mii = Mii::empty(ColorMode::Common);
	mii.hair_type = bytes[0];
	mii.height = low(bytes[1], 7);
	mii.mole_type = bytes[1] >> 7;
	mii.build = low(bytes[2], 7);
	mii.hair_flip = bytes[2] >> 7;
	mii.hair_color = low(bytes[3], 7);
	mii.eye_color = low(bytes[4], 7);
	mii.gender = bytes[4] >> 7;
	mii.eyebrow_color = low(bytes[5], 7);
	mii.mouth_color = low(bytes[6], 7);
	mii.beard_color = low(bytes[7], 7);
	mii.glass_color = low(bytes[8], 7);
	mii.eye_type = low(bytes[9], 6);
	mii.mouth_type = low(bytes[10], 6);
	mii.eye_y = low(bytes[11], 5);
	mii.glass_scale = bytes[11] >> 5;
	mii.eyebrow_type = low(bytes[12], 5);
	mii.mustache_type = bytes[12] >> 5;
	mii.nose_type = low(bytes[13], 5);
	mii.beard_type = bytes[13] >> 5;
	mii.nose_y = low(bytes[14], 5);
	mii.mouth_aspect = bytes[14] >> 5;
	mii.mouth_y = low(bytes[15], 5);
	mii.eyebrow_aspect = bytes[15] >> 5;
	mii.mustache_y = low(bytes[16], 5);
	mii.eye_rotate = bytes[16] >> 5;
	mii.glass_y = low(bytes[17], 5);
	mii.eye_aspect = bytes[17] >> 5;
	mii.mole_x = low(bytes[18], 5);
	mii.eye_scale = bytes[18] >> 5;
	mii.mole_y = low(bytes[19], 5);
	mii.glass_type = low(bytes[20], 5);
	mii.favorite_color = low(bytes[21], 4);
	mii.faceline_type = bytes[21] >> 4;
	mii.faceline_color = low(bytes[22], 4);
	mii.faceline_wrinkle = bytes[22] >> 4;
	mii.faceline_make = low(bytes[23], 4);
	mii.eye_x = bytes[23] >> 4;
	mii.eyebrow_scale = low(bytes[24], 4);
	mii.eyebrow_rotate = bytes[24] >> 4;
	mii.eyebrow_x = low(bytes[25], 4);
	mii.eyebrow_y = (bytes[25] >> 4).saturating_add(3);
	mii.nose_scale = low(bytes[26], 4);
	mii.mouth_scale = bytes[26] >> 4;
	mii.mustache_scale = low(bytes[27], 4);
	mii.mole_scale = bytes[27] >> 4;
	Ok(mii)
}

pub(super) fn decode_ffl_mii_data(bytes: &[u8]) -> Mii {
	let mut mii = Mii::empty(ColorMode::Ver3);
	let personal = u16::from_le_bytes([bytes[24], bytes[25]]);
	mii.gender = bit_range(personal, 0, 1);
	mii.favorite_color = bit_range(personal, 10, 4);
	mii.height = bytes[46];
	mii.build = bytes[47];

	let face = read_u16_le(bytes, 48);
	mii.faceline_type = bit_range(face, 1, 4);
	mii.faceline_color = bit_range(face, 5, 3);
	mii.faceline_wrinkle = bit_range(face, 8, 4);
	mii.faceline_make = bit_range(face, 12, 4);

	let hair = read_u16_le(bytes, 50);
	mii.hair_type = bit_range(hair, 0, 8);
	mii.hair_color = bit_range(hair, 8, 3);
	mii.hair_flip = bit_range(hair, 11, 1);

	let eye = read_u16_le(bytes, 52);
	mii.eye_type = bit_range(eye, 0, 6);
	mii.eye_color = bit_range(eye, 6, 3);
	mii.eye_scale = bit_range(eye, 9, 4);
	mii.eye_aspect = bit_range(eye, 13, 3);

	let eye_position = read_u16_le(bytes, 54);
	mii.eye_rotate = bit_range(eye_position, 0, 5);
	mii.eye_x = bit_range(eye_position, 5, 4);
	mii.eye_y = bit_range(eye_position, 9, 5);

	let eyebrow = read_u16_le(bytes, 56);
	mii.eyebrow_type = bit_range(eyebrow, 0, 5);
	mii.eyebrow_color = bit_range(eyebrow, 5, 3);
	mii.eyebrow_scale = bit_range(eyebrow, 8, 4);
	mii.eyebrow_aspect = bit_range(eyebrow, 12, 3);

	let eyebrow_position = read_u16_le(bytes, 58);
	mii.eyebrow_rotate = bit_range(eyebrow_position, 0, 5);
	mii.eyebrow_x = bit_range(eyebrow_position, 5, 4);
	mii.eyebrow_y = bit_range(eyebrow_position, 9, 5);

	let nose = read_u16_le(bytes, 60);
	mii.nose_type = bit_range(nose, 0, 5);
	mii.nose_scale = bit_range(nose, 5, 4);
	mii.nose_y = bit_range(nose, 9, 5);

	let mouth = read_u16_le(bytes, 62);
	mii.mouth_type = bit_range(mouth, 0, 6);
	mii.mouth_color = bit_range(mouth, 6, 3);
	mii.mouth_scale = bit_range(mouth, 9, 4);
	mii.mouth_aspect = bit_range(mouth, 13, 3);

	let mouth_position = read_u16_le(bytes, 64);
	mii.mouth_y = bit_range(mouth_position, 0, 5);
	mii.mustache_type = bit_range(mouth_position, 5, 3);

	let beard = read_u16_le(bytes, 66);
	mii.beard_type = bit_range(beard, 0, 3);
	mii.beard_color = bit_range(beard, 3, 3);
	mii.mustache_scale = bit_range(beard, 6, 4);
	mii.mustache_y = bit_range(beard, 10, 5);

	let glass = read_u16_le(bytes, 68);
	mii.glass_type = bit_range(glass, 0, 4);
	mii.glass_color = bit_range(glass, 4, 3);
	mii.glass_scale = bit_range(glass, 7, 4);
	mii.glass_y = bit_range(glass, 11, 5);

	let mole = read_u16_le(bytes, 70);
	mii.mole_type = bit_range(mole, 0, 1);
	mii.mole_scale = bit_range(mole, 1, 4);
	mii.mole_x = bit_range(mole, 5, 5);
	mii.mole_y = bit_range(mole, 10, 5);
	mii
}

pub(super) fn decode_rfl_char_data(bytes: &[u8]) -> Result<Mii> {
	let mut mii = Mii::empty(ColorMode::Ver3);
	let personal = read_u16_be(bytes, 0);
	mii.gender = bit_range(personal, 14, 1);
	mii.favorite_color = bit_range(personal, 1, 4);
	mii.height = bytes[22];
	mii.build = bytes[23];

	let face = read_u16_be(bytes, 32);
	mii.faceline_type = bit_range(face, 13, 3);
	mii.faceline_color = bit_range(face, 10, 3);
	let line_and_make = usize::from(bit_range(face, 6, 4));
	let [line, make] =
		RFL_FACE_LINE_AND_MAKE
			.get(line_and_make)
			.copied()
			.ok_or(Error::InvalidMii {
				field: "faceline_line_and_make",
				value: u8::try_from(line_and_make).unwrap_or(u8::MAX),
				min: 0,
				max: 11,
			})?;
	mii.faceline_wrinkle = line;
	mii.faceline_make = make;

	let hair = read_u16_be(bytes, 34);
	mii.hair_type = bit_range(hair, 9, 7);
	mii.hair_color = bit_range(hair, 6, 3);
	mii.hair_flip = bit_range(hair, 5, 1);

	let eyebrow_type = read_u16_be(bytes, 36);
	mii.eyebrow_type = bit_range(eyebrow_type, 11, 5);
	mii.eyebrow_rotate = bit_range(eyebrow_type, 6, 5);
	let eyebrow = read_u16_be(bytes, 38);
	mii.eyebrow_color = bit_range(eyebrow, 13, 3);
	mii.eyebrow_scale = bit_range(eyebrow, 9, 4);
	mii.eyebrow_y = bit_range(eyebrow, 4, 5);
	mii.eyebrow_x = bit_range(eyebrow, 0, 4);
	mii.eyebrow_aspect = 3;

	let eye_type = read_u16_be(bytes, 40);
	mii.eye_type = bit_range(eye_type, 10, 6);
	mii.eye_rotate = bit_range(eye_type, 5, 5);
	mii.eye_y = bit_range(eye_type, 0, 5);
	let eye = read_u16_be(bytes, 42);
	mii.eye_color = bit_range(eye, 13, 3);
	mii.eye_scale = bit_range(eye, 9, 4);
	mii.eye_x = bit_range(eye, 5, 4);
	mii.eye_aspect = 3;

	let nose = read_u16_be(bytes, 44);
	mii.nose_type = bit_range(nose, 12, 4);
	mii.nose_scale = bit_range(nose, 8, 4);
	mii.nose_y = bit_range(nose, 3, 5);

	let mouth = read_u16_be(bytes, 46);
	mii.mouth_type = bit_range(mouth, 11, 5);
	mii.mouth_color = bit_range(mouth, 9, 2);
	mii.mouth_scale = bit_range(mouth, 5, 4);
	mii.mouth_y = bit_range(mouth, 0, 5);
	mii.mouth_aspect = 3;

	let glass = read_u16_be(bytes, 48);
	mii.glass_type = bit_range(glass, 12, 4);
	mii.glass_color = bit_range(glass, 9, 3);
	mii.glass_scale = bit_range(glass, 5, 4);
	mii.glass_y = bit_range(glass, 0, 5);

	let beard = read_u16_be(bytes, 50);
	mii.mustache_type = bit_range(beard, 14, 2);
	mii.beard_type = bit_range(beard, 12, 2);
	mii.beard_color = bit_range(beard, 9, 3);
	mii.mustache_scale = bit_range(beard, 5, 4);
	mii.mustache_y = bit_range(beard, 0, 5);

	let mole = read_u16_be(bytes, 52);
	mii.mole_type = bit_range(mole, 15, 1);
	mii.mole_scale = bit_range(mole, 11, 4);
	mii.mole_y = bit_range(mole, 6, 5);
	mii.mole_x = bit_range(mole, 1, 5);
	Ok(mii)
}

pub(super) fn verify_crc16(bytes: &[u8], format: MiiFormat) -> Result<()> {
	let crc = bytes.iter().fold(0_u16, |mut crc, &byte| {
		for _ in 0..8 {
			crc = if crc & 0x8000 == 0 {
				crc << 1
			} else {
				(crc << 1) ^ 0x1021
			};
		}
		crc ^ u16::from(byte)
	});
	if crc == 0 {
		Ok(())
	} else {
		Err(Error::InvalidMiiChecksum(format))
	}
}

const fn low(value: u8, bits: u32) -> u8 {
	value & ((1_u8 << bits) - 1)
}

fn bit_range(value: u16, shift: u32, bits: u32) -> u8 {
	u8::try_from((value >> shift) & ((1_u16 << bits) - 1)).expect("bit range fits u8")
}

const fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
	u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

const fn read_u16_be(bytes: &[u8], offset: usize) -> u16 {
	u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}
