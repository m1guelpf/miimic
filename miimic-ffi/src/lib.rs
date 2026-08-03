#![warn(clippy::all, clippy::nursery, clippy::pedantic)]
#![allow(clippy::used_underscore_items, reason = "boltffi macros generate used underscore items")]

use boltffi::export;
use std::sync::{Mutex, MutexGuard, PoisonError};

macro_rules! impl_numeric_mirror {
	($mirror:ident => $domain:path) => {
		impl From<$mirror> for $domain {
			fn from(value: $mirror) -> Self {
				let value = u8::from(value);
				<Self>::try_from(value).expect(concat!(
					stringify!($mirror),
					" must have the same discriminants as ",
					stringify!($domain),
				))
			}
		}
	};
}

#[repr(u8)]
#[boltffi::data]
#[derive(
	Clone, Copy, Debug, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive, PartialEq,
)]
/// Encoded output format produced by a reusable renderer.
pub enum OutputFormat {
	Png = 0,
	Tga = 1,
	Glb = 2,
}

/// Scene and camera view used for rendering.
#[repr(u8)]
#[boltffi::data]
#[derive(
	Clone, Copy, Debug, Default, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive, PartialEq,
)]
pub enum ViewType {
	/// Face-focused view with the head attached to the Wii U body.
	#[default]
	Avatar = 0,
	/// Standalone head using the same face-focused framing.
	Face = 1,
	/// Complete Wii U body with the head attached and centered in frame.
	Body = 2,
}

/// Facial expression rendered on a Mii.
#[boltffi::data]
#[derive(
	Clone, Copy, Debug, Default, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive, PartialEq,
)]
#[repr(u8)]
pub enum Expression {
	#[default]
	Normal = 0,
	Smile = 1,
	Anger = 2,
	Sorrow = 3,
	Surprise = 4,
	Blink = 5,
	OpenMouth = 6,
	SmileOpenMouth = 7,
	AngerOpenMouth = 8,
	SorrowOpenMouth = 9,
	SurpriseOpenMouth = 10,
	BlinkOpenMouth = 11,
	WinkLeft = 12,
	WinkRight = 13,
	WinkLeftOpenMouth = 14,
	WinkRightOpenMouth = 15,
	LikeWinkLeft = 16,
	LikeWinkRight = 17,
	Frustrated = 18,
	Bored = 19,
	BoredOpenMouth = 20,
	SighMouthStraight = 21,
	Sigh = 22,
	DisgustedMouthStraight = 23,
	Disgusted = 24,
	Love = 25,
	LoveOpenMouth = 26,
	DeterminedMouthStraight = 27,
	Determined = 28,
	CryMouthStraight = 29,
	Cry = 30,
	BigSmileMouthStraight = 31,
	BigSmile = 32,
	Cheeky = 33,
	CheekyDuplicate = 34,
	Afl35 = 35,
	Afl36 = 36,
	Smug = 37,
	SmugOpenMouth = 38,
	Resolve = 39,
	ResolveOpenMouth = 40,
	Unbelievable = 41,
	UnbelievableDuplicate = 42,
	Cunning = 43,
	CunningDuplicate = 44,
	Raspberry = 45,
	RaspberryDuplicate = 46,
	Innocent = 47,
	InnocentDuplicate = 48,
	Cat = 49,
	CatDuplicate = 50,
	Dog = 51,
	DogDuplicate = 52,
	Tasty = 53,
	TastyDuplicate = 54,
	MoneyMouthStraight = 55,
	Money = 56,
	ConfusedMouthStraight = 57,
	Confused = 58,
	CheerfulMouthStraight = 59,
	Cheerful = 60,
	Afl61 = 61,
	Afl62 = 62,
	GrumbleMouthStraight = 63,
	Grumble = 64,
	MovedMouthStraight = 65,
	Moved = 66,
	SingingMouthSmall = 67,
	Singing = 68,
	Stunned = 69,
}

impl_numeric_mirror!(ViewType => miimic::ViewType);
impl_numeric_mirror!(Expression => miimic::Expression);
impl_numeric_mirror!(OutputFormat => miimic::OutputFormat);

/// A validated FFL resource archive owned by Rust.
#[derive(Clone)]
pub struct ResourceArchive {
	inner: miimic::ResourceArchive,
}

#[export]
impl ResourceArchive {
	/// Opens and validates an FFL resource archive.
	///
	/// # Errors
	///
	/// Returns an error if the archive cannot be opened or fails validation.
	pub fn open(path: &str) -> Result<Self, String> {
		miimic::ResourceArchive::open(path)
			.map(|inner| Self { inner })
			.map_err(|error| error.to_string())
	}
}

/// A validated render request owned by Rust.
pub struct RenderRequest {
	inner: Mutex<miimic::RenderRequest>,
}

#[export]
impl RenderRequest {
	/// Decodes Mii data and creates a request with width-derived defaults.
	///
	/// # Errors
	///
	/// Returns an error if the encoded Mii data or width is invalid.
	pub fn new(encoded_mii: &str, width: u16) -> Result<Self, String> {
		let mii = miimic::MiiData::decode(encoded_mii).map_err(|error| error.to_string())?;
		let request = miimic::RenderRequest::new(mii, width).map_err(|error| error.to_string())?;

		Ok(Self {
			inner: Mutex::new(request),
		})
	}

	/// Overrides the generated face-mask texture resolution.
	///
	/// # Errors
	///
	/// Returns an error if the resolution is outside the supported range.
	pub fn set_texture_resolution(&self, texture_resolution: u16) -> Result<(), String> {
		self.request()
			.set_texture_resolution(texture_resolution)
			.map_err(|error| error.to_string())
	}

	/// Selects the facial expression rendered on the Mii.
	pub fn set_expression(&self, expression: Expression) {
		self.request().set_expression(expression.into());
	}

	/// Selects the camera framing used for raster output.
	pub fn set_view_type(&self, view_type: ViewType) {
		self.request().set_view_type(view_type.into());
	}

	/// Returns the generated face-mask texture resolution.
	pub fn texture_resolution(&self) -> u16 {
		self.request().texture_resolution()
	}
}

impl RenderRequest {
	fn request(&self) -> MutexGuard<'_, miimic::RenderRequest> {
		self.inner.lock().unwrap_or_else(PoisonError::into_inner)
	}

	fn snapshot(&self) -> miimic::RenderRequest {
		self.request().clone()
	}
}

/// Renders one PNG image without retaining renderer state.
///
/// # Errors
///
/// Returns an error if renderer initialization, rendering, or encoding fails.
#[export]
pub fn render_to_png(
	resources: &ResourceArchive,
	request: &RenderRequest,
) -> Result<Vec<u8>, String> {
	miimic::render_to_png(&resources.inner, &request.snapshot()).map_err(|error| error.to_string())
}

/// Exports one binary glTF model without initializing a GPU renderer.
///
/// # Errors
///
/// Returns an error if model construction or encoding fails.
#[export]
pub fn render_to_glb(
	resources: &ResourceArchive,
	request: &RenderRequest,
) -> Result<Vec<u8>, String> {
	miimic::render_to_glb(&resources.inner, &request.snapshot()).map_err(|error| error.to_string())
}

/// A reusable renderer that retains resource and GPU state between calls.
pub struct Renderer {
	inner: miimic::Renderer,
}

#[export]
impl Renderer {
	/// Creates a reusable renderer from an open resource archive.
	///
	/// # Errors
	///
	/// Returns an error if GPU initialization fails.
	pub fn new(resources: &ResourceArchive) -> Result<Self, String> {
		miimic::Renderer::new(resources.inner.clone())
			.map(|inner| Self { inner })
			.map_err(|error| error.to_string())
	}

	/// Produces an encoded PNG, TGA, or GLB byte vector.
	///
	/// # Errors
	///
	/// Returns an error if rendering or encoding fails.
	pub fn render(&self, request: &RenderRequest, output: OutputFormat) -> Result<Vec<u8>, String> {
		self.inner
			.render(&request.snapshot(), output.into())
			.map_err(|error| error.to_string())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const TEST_MII: &str = "0800600308040402020c0301060406020a0000000008000804000a2a003e7f04000214031304160d04000a040109";

	macro_rules! assert_numeric_mirror {
		($mirror:ty => $domain:ty, $values:expr) => {
			for value in $values {
				let mirror = <$mirror>::try_from(value).expect("mirror discriminant should exist");
				let mirror_name = format!("{mirror:?}");
				let domain: $domain = mirror.into();
				assert_eq!(u8::from(domain), value);
				assert_eq!(format!("{domain:?}"), mirror_name);
			}
		};
	}

	#[test]
	fn mirror_enums_match_the_library_domain_enums() {
		assert_numeric_mirror!(ViewType => miimic::ViewType, 0..=2);
		assert_numeric_mirror!(Expression => miimic::Expression, 0..=69);
		assert_numeric_mirror!(OutputFormat => miimic::OutputFormat, 0..=2);
	}

	#[test]
	fn request_constructor_and_setters_preserve_validation() {
		let request = RenderRequest::new(TEST_MII, 512).expect("request should validate");
		assert_eq!(request.texture_resolution(), 512);
		assert!(request.set_texture_resolution(1).is_err());
		request
			.set_texture_resolution(1024)
			.expect("supported resolution should validate");
		assert_eq!(request.texture_resolution(), 1024);
		request.set_expression(Expression::Stunned);
		request.set_view_type(ViewType::Face);
	}
}
