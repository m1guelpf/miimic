use crate::{Error, MiiData, MiiFormat, Result};

mod format;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorMode {
	Common,
	Ver3,
}

#[derive(Clone, Copy)]
struct StudioFieldRule {
	offset: usize,
	name: &'static str,
	min: u8,
	max: u8,
}

macro_rules! define_studio_schema {
	($(($offset:literal, $field:ident, $min:literal, $max:literal)),+ $(,)?) => {
		const STUDIO_FIELD_RULES: &[StudioFieldRule] = &[
			$(StudioFieldRule {
				offset: $offset,
				name: stringify!($field),
				min: $min,
				max: $max,
			}),+
		];

		/// Renderer-facing Mii parameters.
		///
		/// The field order and values match Nintendo's 46-byte Mii Studio structure.
		/// Studio colors are Switch common-color table indices.
			#[derive(Clone, Copy, Debug, Eq, PartialEq)]
			pub struct Mii {
				$(pub(super) $field: u8),+,
				pub(super) color_mode: ColorMode,
			}

			impl Mii {
				const fn from_studio_bytes(bytes: &[u8; 46]) -> Self {
					Self::from_fields(bytes, ColorMode::Common)
				}

				const fn from_fields(bytes: &[u8; 46], color_mode: ColorMode) -> Self {
					Self {
						$($field: bytes[$offset]),+,
						color_mode,
					}
				}

				const fn empty(color_mode: ColorMode) -> Self {
					Self::from_fields(&[0; 46], color_mode)
				}

				const fn field_values(&self) -> [u8; 46] {
					[$(self.$field),+]
				}
			}
	};
}

// These are source-format limits. Expression-only textures and wider archive
// tables are checked separately and must not widen accepted Studio bytes.
define_studio_schema!(
	(0, beard_color, 0, 99),
	(1, beard_type, 0, 5),
	(2, build, 0, 127),
	(3, eye_aspect, 0, 6),
	(4, eye_color, 0, 99),
	(5, eye_rotate, 0, 7),
	(6, eye_scale, 0, 7),
	(7, eye_type, 0, 59),
	(8, eye_x, 0, 12),
	(9, eye_y, 0, 18),
	(10, eyebrow_aspect, 0, 6),
	(11, eyebrow_color, 0, 99),
	(12, eyebrow_rotate, 0, 11),
	(13, eyebrow_scale, 0, 8),
	(14, eyebrow_type, 0, 23),
	(15, eyebrow_x, 0, 12),
	(16, eyebrow_y, 3, 18),
	(17, faceline_color, 0, 9),
	(18, faceline_make, 0, 11),
	(19, faceline_type, 0, 11),
	(20, faceline_wrinkle, 0, 11),
	(21, favorite_color, 0, 11),
	(22, gender, 0, 1),
	(23, glass_color, 0, 99),
	(24, glass_scale, 0, 7),
	(25, glass_type, 0, 19),
	(26, glass_y, 0, 20),
	(27, hair_color, 0, 99),
	(28, hair_flip, 0, 1),
	(29, hair_type, 0, 131),
	(30, height, 0, 127),
	(31, mole_scale, 0, 8),
	(32, mole_type, 0, 1),
	(33, mole_x, 0, 16),
	(34, mole_y, 0, 30),
	(35, mouth_aspect, 0, 6),
	(36, mouth_color, 0, 99),
	(37, mouth_scale, 0, 8),
	(38, mouth_type, 0, 35),
	(39, mouth_y, 0, 18),
	(40, mustache_scale, 0, 8),
	(41, mustache_type, 0, 5),
	(42, mustache_y, 0, 16),
	(43, nose_scale, 0, 8),
	(44, nose_type, 0, 17),
	(45, nose_y, 0, 18),
);

impl Mii {
	/// Converts serialized Mii data into renderer-facing parameters.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidMii`] if a field is out of range, or
	/// [`Error::InvalidMiiChecksum`] if a checksummed representation is corrupt.
	pub(super) fn from_data(data: &MiiData) -> Result<Self> {
		let bytes = data.as_bytes();
		let mii = match data.format() {
			MiiFormat::Studio => {
				let bytes: &[u8; 46] = bytes
					.try_into()
					.map_err(|_| Error::UnsupportedDataLength(data.len()))?;
				validate_studio_fields(bytes)?;
				Self::from_studio_bytes(bytes)
			},
			MiiFormat::StudioUrl => {
				let decoded = format::decode_studio_url(bytes)?;
				validate_studio_fields(&decoded)?;
				Self::from_studio_bytes(&decoded)
			},
			MiiFormat::SwitchCoreData | MiiFormat::SwitchStoreData => {
				format::decode_switch_core_data(bytes)?
			},
			MiiFormat::SwitchCharInfo => format::decode_switch_char_info(bytes)?,
			MiiFormat::FflMiiDataCore | MiiFormat::FflMiiDataOfficial => {
				format::decode_ffl_mii_data(bytes)
			},
			MiiFormat::FflStoreData => {
				format::verify_crc16(bytes, data.format())?;
				format::decode_ffl_mii_data(bytes)
			},
			MiiFormat::RflCharData => format::decode_rfl_char_data(bytes)?,
			MiiFormat::RflStoreData => {
				format::verify_crc16(bytes, data.format())?;
				format::decode_rfl_char_data(bytes)?
			},
		};

		validate_decoded_fields(&mii)?;
		Ok(mii)
	}
}

impl TryFrom<&MiiData> for Mii {
	type Error = Error;

	fn try_from(data: &MiiData) -> Result<Self> {
		Self::from_data(data)
	}
}

impl TryFrom<MiiData> for Mii {
	type Error = Error;

	fn try_from(data: MiiData) -> Result<Self> {
		Self::from_data(&data)
	}
}

fn validate_studio_fields(bytes: &[u8; 46]) -> Result<()> {
	for rule in STUDIO_FIELD_RULES {
		let value = bytes[rule.offset];
		if !(rule.min..=rule.max).contains(&value) {
			return Err(Error::InvalidMii {
				field: rule.name,
				value,
				min: rule.min,
				max: rule.max,
			});
		}
	}
	Ok(())
}

fn validate_decoded_fields(mii: &Mii) -> Result<()> {
	let values = mii.field_values();
	for rule in STUDIO_FIELD_RULES {
		let value = values[rule.offset];
		let max = decoded_field_max(rule.offset, rule.max, mii.color_mode);
		if !(rule.min..=max).contains(&value) {
			return Err(Error::InvalidMii {
				field: rule.name,
				value,
				min: rule.min,
				max,
			});
		}
	}
	Ok(())
}

const fn decoded_field_max(offset: usize, studio_max: u8, color_mode: ColorMode) -> u8 {
	match (offset, color_mode) {
		(2 | 30, _) => 128,
		(14, _) => 27,
		(0 | 11 | 27, ColorMode::Ver3) => 7,
		(4 | 23, ColorMode::Ver3) => 5,
		(25, ColorMode::Ver3) => 8,
		(36, ColorMode::Ver3) => 4,
		_ => studio_max,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const TEST_MII: &str = "0800600308040402020c0301060406020a0000000008000804000a2a003e7f04000214031304160d04000a040109";
	const STUDIO_URL: &str = "5a5960070b0a1518212a2d353b44474851626970777e7d84939ea5b6a3aa9bebf6fd0619213944595b666d6e717785";
	const SWITCH_CORE: &str = "3e7f602a080113080802168c0601696d8a6a821400080020647244444d00000000000000000000000000000000000000";
	const SWITCH_STORE: &str = "3e7f602a080113080802168c0601696d8a6a821400080020647244444d000000000000000000000000000000000000000000000000000000000000000000000000000000";
	const SWITCH_CHAR_INFO: &str = "000000000000000000000000000000004d0000000000000000000000000000000000000000000008007f600000000000003e2a000208040304020c0601040306020a010409161304030d080000040a0008040a0004021400";
	const FFL_CORE: &str = "03000010000000000000000000000000000000000000000000106e006f0020006e0061006d0065000000000000004040810044000268441806344614811217680d00002900524850";
	const FFL_OFFICIAL: &str = "03000010000000000000000000000000000000000000000000106e006f0020006e0061006d0065000000000000004040810044000268441806344614811217680d000029005248500000000000000000000000000000000000000000";
	const FFL_STORE: &str = "03000010000000000000000000000000000000000000000000106e006f0020006e0061006d0065000000000000004040810044000268441806344614811217680d0000290052485000000000000000000000000000000000000000000000fac7";
	const RFL_CHAR: &str = "0008006e006f0020006e0061006d00650000000000004040000000000000000010048800318008a2088c0858144ab88d008a008a25040000000000000000000000000000000000000000";
	const RFL_STORE: &str = "0008006e006f0020006e0061006d00650000000000004040000000000000000010048800318008a2088c0858144ab88d008a008a25040000000000000000000000000000000000000000b891";

	#[test]
	fn decodes_canonical_studio_fields() {
		let data = MiiData::decode(TEST_MII).expect("fixture should decode");
		let mii = Mii::from_data(&data).expect("fixture should validate");
		assert_eq!(mii.build, 96);
		assert_eq!(mii.eye_type, 2);
		assert_eq!(mii.hair_color, 42);
		assert_eq!(mii.hair_type, 62);
		assert_eq!(mii.height, 127);
		assert_eq!(mii.mouth_type, 22);
		assert_eq!(mii.nose_type, 1);
	}

	#[test]
	fn switch_and_encoded_studio_formats_match_raw_studio() {
		let expected = decode_mii(TEST_MII);
		for encoded in [STUDIO_URL, SWITCH_CORE, SWITCH_STORE, SWITCH_CHAR_INFO] {
			assert_eq!(decode_mii(encoded), expected);
		}
	}

	#[test]
	fn ffl_and_rfl_formats_match_the_reference_default_mii() {
		let expected = decode_mii(RFL_CHAR);
		assert_eq!(expected.color_mode, ColorMode::Ver3);
		assert_eq!(expected.height, 64);
		assert_eq!(expected.build, 64);
		assert_eq!(expected.hair_type, 68);
		assert_eq!(expected.faceline_color, 4);
		assert_eq!(expected.eye_type, 2);

		for encoded in [RFL_STORE, FFL_CORE, FFL_OFFICIAL, FFL_STORE] {
			assert_eq!(decode_mii(encoded), expected);
		}
	}

	#[test]
	fn rejects_bad_checksums_without_rejecting_non_store_containers() {
		for encoded in [RFL_STORE, FFL_STORE] {
			let mut bytes = hex::decode(encoded).expect("fixture should be hexadecimal");
			bytes[0] ^= 1;
			let data = MiiData::try_from(bytes).expect("fixture length should remain recognized");
			assert!(matches!(
				Mii::from_data(&data),
				Err(Error::InvalidMiiChecksum(format)) if format == data.format()
			));
		}
	}

	#[test]
	fn schema_covers_each_studio_byte_in_order() {
		for (offset, rule) in STUDIO_FIELD_RULES.iter().enumerate() {
			assert_eq!(rule.offset, offset, "misordered rule for {}", rule.name);
		}
	}

	#[test]
	fn every_studio_field_accepts_exactly_its_documented_byte_range() {
		let baseline = MiiData::decode(TEST_MII).expect("fixture should decode");
		for rule in STUDIO_FIELD_RULES {
			for value in u8::MIN..=u8::MAX {
				let mut bytes = baseline.as_bytes().to_vec();
				bytes[rule.offset] = value;
				let data = MiiData::try_from(bytes).expect("Studio data length should be valid");
				let result = Mii::from_data(&data);
				let expected_valid = (rule.min..=rule.max).contains(&value);
				match result {
					Ok(_) => assert!(
						expected_valid,
						"{}={value} should have been rejected",
						rule.name
					),
					Err(Error::InvalidMii {
						field,
						value: rejected,
						min,
						max,
					}) => {
						assert!(
							!expected_valid,
							"{}={value} should have been accepted",
							rule.name
						);
						assert_eq!(field, rule.name);
						assert_eq!(rejected, value);
						assert_eq!((min, max), (rule.min, rule.max));
					},
					Err(error) => {
						panic!("{}={value} returned unexpected error: {error}", rule.name)
					},
				}
			}
		}
	}

	#[test]
	fn reports_structured_invalid_field_details() {
		let baseline = MiiData::decode(TEST_MII).expect("fixture should decode");
		let mut bytes = baseline.as_bytes().to_vec();
		bytes[4] = u8::MAX;
		let data = MiiData::try_from(bytes).expect("Studio data length should be valid");
		assert!(matches!(
			Mii::from_data(&data),
			Err(Error::InvalidMii {
				field: "eye_color",
				value: u8::MAX,
				min: 0,
				max: 99,
			})
		));
	}

	fn decode_mii(encoded: &str) -> Mii {
		let data = MiiData::decode(encoded).expect("fixture should decode");
		Mii::from_data(&data).expect("fixture should contain valid Mii fields")
	}
}
