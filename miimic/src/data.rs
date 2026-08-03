use base64::{Engine as _, engine::general_purpose};

use crate::{Error, Result};

/// A recognized serialized Mii representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MiiFormat {
	Studio,
	StudioUrl,
	SwitchCoreData,
	FflMiiDataCore,
	RflCharData,
	RflStoreData,
	SwitchStoreData,
	SwitchCharInfo,
	FflMiiDataOfficial,
	FflStoreData,
}

impl MiiFormat {
	#[must_use]
	const fn from_len(len: usize) -> Option<Self> {
		match len {
			46 => Some(Self::Studio),
			47 => Some(Self::StudioUrl),
			48 => Some(Self::SwitchCoreData),
			68 => Some(Self::SwitchStoreData),
			72 => Some(Self::FflMiiDataCore),
			74 => Some(Self::RflCharData),
			76 => Some(Self::RflStoreData),
			88 => Some(Self::SwitchCharInfo),
			92 => Some(Self::FflMiiDataOfficial),
			96 => Some(Self::FflStoreData),
			_ => None,
		}
	}
}

/// Length-validated serialized Mii data together with its detected representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MiiData {
	bytes: Box<[u8]>,
	format: MiiFormat,
}

impl MiiData {
	/// Decodes a hexadecimal or base64 representation and validates its size.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidEncoding`] when the text is neither supported
	/// hexadecimal nor Base64, and [`Error::UnsupportedDataLength`] when the
	/// decoded bytes do not match a recognized Mii representation.
	pub fn decode(encoded: &str) -> Result<Self> {
		let encoded = encoded.trim();
		if encoded.is_empty() {
			return Err(Error::InvalidEncoding);
		}

		let hex_length = if encoded.len().is_multiple_of(2)
			&& encoded.bytes().all(|byte| byte.is_ascii_hexdigit())
		{
			let bytes = hex::decode(encoded).map_err(|_| Error::InvalidEncoding)?;
			if MiiFormat::from_len(bytes.len()).is_some() {
				return Self::try_from(bytes);
			}
			Some(bytes.len())
		} else {
			None
		};

		let decoded = [
			&general_purpose::STANDARD,
			&general_purpose::STANDARD_NO_PAD,
			&general_purpose::URL_SAFE,
			&general_purpose::URL_SAFE_NO_PAD,
		]
		.into_iter()
		.find_map(|engine| engine.decode(encoded).ok())
		.ok_or_else(|| hex_length.map_or(Error::InvalidEncoding, Error::UnsupportedDataLength))?;

		Self::try_from(decoded)
	}

	#[must_use]
	pub const fn format(&self) -> MiiFormat {
		self.format
	}

	#[must_use]
	pub(super) fn as_bytes(&self) -> &[u8] {
		&self.bytes
	}

	#[must_use]
	pub fn len(&self) -> usize {
		self.bytes.len()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.bytes.is_empty()
	}
}

impl TryFrom<Vec<u8>> for MiiData {
	type Error = Error;

	fn try_from(bytes: Vec<u8>) -> Result<Self> {
		let format =
			MiiFormat::from_len(bytes.len()).ok_or(Error::UnsupportedDataLength(bytes.len()))?;
		Ok(Self {
			bytes: bytes.into_boxed_slice(),
			format,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const TEST_MII: &str = "0800600308040402020c0301060406020a0000000008000804000a2a003e7f04000214031304160d04000a040109";

	#[test]
	fn detects_canonical_test_mii_as_studio_data() {
		let data = MiiData::decode(TEST_MII).expect("canonical fixture should decode");
		assert_eq!(data.len(), 46);
		assert_eq!(data.format(), MiiFormat::Studio);
	}

	#[test]
	fn accepts_unpadded_base64() {
		let encoded = general_purpose::STANDARD_NO_PAD.encode([0_u8; 46]);
		let data = MiiData::decode(&encoded).expect("base64 should decode");
		assert_eq!(data.format(), MiiFormat::Studio);
	}

	#[test]
	fn rejects_unknown_lengths() {
		let error = MiiData::try_from(vec![0; 45]).expect_err("45 bytes is unsupported");
		assert!(matches!(error, Error::UnsupportedDataLength(45)));
	}

	#[test]
	fn detects_every_supported_representation_by_exact_length() {
		let formats = [
			(46, MiiFormat::Studio),
			(47, MiiFormat::StudioUrl),
			(48, MiiFormat::SwitchCoreData),
			(68, MiiFormat::SwitchStoreData),
			(72, MiiFormat::FflMiiDataCore),
			(74, MiiFormat::RflCharData),
			(76, MiiFormat::RflStoreData),
			(88, MiiFormat::SwitchCharInfo),
			(92, MiiFormat::FflMiiDataOfficial),
			(96, MiiFormat::FflStoreData),
		];
		for (length, expected) in formats {
			let data = MiiData::try_from(vec![0; length]).expect("length should be recognized");
			assert_eq!(data.format(), expected);
		}
	}
}
