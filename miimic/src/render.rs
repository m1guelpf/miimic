use std::path::Path;

use crate::{
	CharacterModel, Error, Expression, Mii, MiiData, ResourceArchive, Result, body::BodyAssets,
	encode_glb, raster::Rasterizer,
};

/// Largest square raster output accepted by Miimic.
const MAX_RENDER_WIDTH: u16 = 1080;
/// Smallest generated face-mask texture accepted by Miimic.
const MIN_TEXTURE_RESOLUTION: u16 = 2;
/// Largest generated face-mask texture accepted by Miimic.
const MAX_TEXTURE_RESOLUTION: u16 = 1024;

/// Encoded output format produced by a reusable [`Renderer`].
#[derive(
	Clone, Copy, Debug, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive, PartialEq,
)]
#[repr(u8)]
pub enum OutputFormat {
	Png = 0,
	Tga = 1,
	Glb = 2,
}

/// Scene and camera view used for rendering.
#[derive(
	Clone, Copy, Debug, Default, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive, PartialEq,
)]
#[repr(u8)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ViewType {
	/// Face-focused view with the head attached to the Wii U body.
	#[default]
	Avatar = 0,
	/// Standalone head using the same face-focused framing.
	Face = 1,
	/// Complete Wii U body with the head attached and centered in frame.
	Body = 2,
}

impl OutputFormat {
	#[must_use]
	pub const fn media_type(self) -> &'static str {
		match self {
			Self::Png => "image/png",
			Self::Tga => "image/tga",
			Self::Glb => "model/gltf-binary",
		}
	}

	#[must_use]
	pub const fn extension(self) -> &'static str {
		match self {
			Self::Png => "png",
			Self::Tga => "tga",
			Self::Glb => "glb",
		}
	}
}

/// Validated Mii and options shared by one-shot and reusable rendering.
#[derive(Clone, Debug)]
pub struct RenderRequest {
	mii: Mii,
	width: u16,
	texture_resolution: u16,
	expression: Expression,
	background: [u8; 4],
	view_type: ViewType,
}

impl RenderRequest {
	/// Creates a render request using bounded HD defaults.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidMii`] when the serialized Mii has an invalid
	/// field, [`Error::InvalidMiiChecksum`] when checksummed data is corrupt, or
	/// [`Error::InvalidRequest`] when `width` is zero or exceeds 1080 pixels.
	pub fn new(mii: MiiData, width: u16) -> Result<Self> {
		validate_width(width)?;

		Ok(Self {
			mii: Mii::try_from(mii)?,
			width,
			texture_resolution: width.clamp(MIN_TEXTURE_RESOLUTION, MAX_TEXTURE_RESOLUTION),
			expression: Expression::Normal,
			// FFL-Testing uses transparent white because transparent RGB still
			// affects partially transparent surfaces such as glasses.
			background: [u8::MAX, u8::MAX, u8::MAX, 0],
			view_type: ViewType::Avatar,
		})
	}

	/// Overrides the generated face-mask texture resolution.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidRequest`] when `texture_resolution` is outside
	/// 2 through 1024 pixels.
	pub fn set_texture_resolution(&mut self, texture_resolution: u16) -> Result<()> {
		validate_texture_resolution(texture_resolution)?;
		self.texture_resolution = texture_resolution;
		Ok(())
	}

	/// Overrides the expression rendered on the Mii.
	pub const fn set_expression(&mut self, expression: Expression) {
		self.expression = expression;
	}

	/// Overrides the scene and camera view used for rendering.
	pub const fn set_view_type(&mut self, view_type: ViewType) {
		self.view_type = view_type;
	}

	/// Returns the generated face-mask texture resolution.
	#[must_use]
	pub const fn texture_resolution(&self) -> u16 {
		self.texture_resolution
	}
}

fn validate_width(width: u16) -> Result<()> {
	if width == 0 || width > MAX_RENDER_WIDTH {
		return Err(Error::InvalidRequest(format!(
			"width must be between 1 and {MAX_RENDER_WIDTH}"
		)));
	}
	Ok(())
}

pub fn validate_texture_resolution(texture_resolution: u16) -> Result<()> {
	if !(MIN_TEXTURE_RESOLUTION..=MAX_TEXTURE_RESOLUTION).contains(&texture_resolution) {
		return Err(Error::InvalidRequest(format!(
			"texture resolution must be between {MIN_TEXTURE_RESOLUTION} and {MAX_TEXTURE_RESOLUTION}"
		)));
	}
	Ok(())
}

/// Renders one PNG image without retaining renderer state.
///
/// Use [`Renderer`] instead when rendering more than once so GPU and body
/// resources are initialized only once.
///
/// # Errors
///
/// Returns an error when model construction, renderer initialization,
/// rasterization, or PNG encoding fails.
pub fn render_to_png(resources: &ResourceArchive, request: &RenderRequest) -> Result<Vec<u8>> {
	let body_assets = if request.view_type.uses_body() {
		load_body_assets(resources)?
	} else {
		None
	};
	let rasterizer = Rasterizer::new()?;
	render_raster(
		resources,
		body_assets.as_ref(),
		&rasterizer,
		request,
		OutputFormat::Png,
	)
}

/// Exports one binary glTF model without initializing a GPU renderer.
///
/// # Errors
///
/// Returns an error when model construction, optional body construction, or
/// GLB encoding fails.
pub fn render_to_glb(resources: &ResourceArchive, request: &RenderRequest) -> Result<Vec<u8>> {
	let body_assets = if request.view_type == ViewType::Body {
		load_body_assets(resources)?
	} else {
		None
	};
	render_glb(resources, body_assets.as_ref(), request)
}

fn render_glb(
	resources: &ResourceArchive,
	body_assets: Option<&BodyAssets>,
	request: &RenderRequest,
) -> Result<Vec<u8>> {
	let model = build_model(resources, request)?;
	if request.view_type != ViewType::Body {
		return encode_glb(&model);
	}
	let body = body_assets
		.ok_or(Error::FeatureUnavailable("Wii U body assets"))?
		.build(&model.mii)?;
	crate::encode_glb_with_body(&model, &body)
}

fn build_model(resources: &ResourceArchive, request: &RenderRequest) -> Result<CharacterModel> {
	CharacterModel::build(
		resources,
		request.mii,
		request.expression,
		request.texture_resolution,
	)
}

fn load_body_assets(resources: &ResourceArchive) -> Result<Option<BodyAssets>> {
	let Some(path) = resources.source_path() else {
		return Ok(None);
	};
	BodyAssets::near_resource(path)
}

fn render_raster(
	resources: &ResourceArchive,
	body_assets: Option<&BodyAssets>,
	rasterizer: &Rasterizer,
	request: &RenderRequest,
	output: OutputFormat,
) -> Result<Vec<u8>> {
	let model = build_model(resources, request)?;
	let body = if request.view_type.uses_body() {
		Some(
			body_assets
				.ok_or(Error::FeatureUnavailable("Wii U body assets"))?
				.build(&model.mii)?,
		)
	} else {
		None
	};
	rasterizer.render(
		&model,
		body.as_ref(),
		request.view_type,
		request.width,
		request.background,
		output,
	)
}

/// Pure-Rust renderer backed by one immutable FFL resource archive.
#[derive(Clone, Debug)]
pub struct Renderer {
	resources: ResourceArchive,
	body_assets: Option<BodyAssets>,
	rasterizer: std::sync::Arc<Rasterizer>,
}

impl Renderer {
	/// Creates a reusable renderer from a validated resource archive.
	///
	/// # Errors
	///
	/// Returns an error when nearby body assets are invalid or a GPU renderer
	/// cannot be initialized.
	pub fn new(resources: ResourceArchive) -> Result<Self> {
		Ok(Self {
			body_assets: load_body_assets(&resources)?,
			resources,
			rasterizer: std::sync::Arc::new(Rasterizer::new()?),
		})
	}

	/// Opens the resource archive used by subsequent renders.
	///
	/// # Errors
	///
	/// Returns an error if the archive cannot be read or validated.
	pub fn open(path: impl AsRef<Path>) -> Result<Self> {
		Self::new(ResourceArchive::open(path)?)
	}

	/// Produces encoded bytes in `output` format.
	///
	/// # Errors
	///
	/// Returns an error when input validation, model construction, rendering,
	/// or output encoding fails.
	pub fn render(&self, request: &RenderRequest, output: OutputFormat) -> Result<Vec<u8>> {
		let bytes = match output {
			OutputFormat::Glb => render_glb(&self.resources, self.body_assets.as_ref(), request)?,
			OutputFormat::Png | OutputFormat::Tga => render_raster(
				&self.resources,
				self.body_assets.as_ref(),
				&self.rasterizer,
				request,
				output,
			)?,
		};
		Ok(bytes)
	}
}

impl ViewType {
	const fn uses_body(self) -> bool {
		matches!(self, Self::Avatar | Self::Body)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const TEST_MII: &str = "0800600308040402020c0301060406020a0000000008000804000a2a003e7f04000214031304160d04000a040109";

	#[test]
	fn render_dimensions_use_bounded_hd_defaults() {
		for (width, expected_texture_resolution) in
			[(1, MIN_TEXTURE_RESOLUTION), (512, 512), (1080, 1024)]
		{
			let request = RenderRequest::new(
				MiiData::decode(TEST_MII).expect("fixture should decode"),
				width,
			)
			.expect("bounded width should validate");
			assert_eq!(request.texture_resolution(), expected_texture_resolution);
		}
	}

	#[test]
	fn render_options_reject_values_outside_the_supported_profile() {
		let mii = MiiData::decode(TEST_MII).expect("fixture should decode");
		assert!(RenderRequest::new(mii.clone(), 0).is_err());
		assert!(RenderRequest::new(mii.clone(), 1081).is_err());

		let mut request = RenderRequest::new(mii, 512).expect("request should validate");
		assert!(request.set_texture_resolution(1).is_err());
		assert!(request.set_texture_resolution(1025).is_err());
		assert_eq!(request.texture_resolution(), 512);
		request.set_expression(Expression::Stunned);
		assert_eq!(request.expression, Expression::Stunned);
	}

	#[test]
	fn expression_numeric_conversion_covers_the_supported_range() {
		for value in 0..70 {
			let expression = Expression::try_from(value).expect("expression should convert");
			assert_eq!(u8::from(expression), value);
		}
		assert!(Expression::try_from(70).is_err());
	}

	#[test]
	fn width_errors_precede_mii_field_validation() {
		let mut bytes = MiiData::decode(TEST_MII)
			.expect("fixture should decode")
			.as_bytes()
			.to_vec();
		bytes[4] = u8::MAX;
		let invalid_mii = MiiData::try_from(bytes).expect("Studio data length should remain valid");
		let error =
			RenderRequest::new(invalid_mii, 0).expect_err("invalid width should be reported first");
		assert!(matches!(error, Error::InvalidRequest(_)));
	}
}
