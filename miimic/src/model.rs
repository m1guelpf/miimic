use crate::{
	Expression, Mii, ResourceArchive, Result, Shape, ShapePart, ShapeTransform, Texture,
	TextureFormat, TexturePart, color, faceline, mask,
};

/// Material operation attached to a model mesh.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModulateMode {
	Constant,
	TextureDirect,
	Alpha,
	LuminanceAlpha,
}

/// Semantic FFL part represented by a model mesh.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelPart {
	Faceline,
	Beard,
	Nose,
	Forehead,
	Hair,
	Mask,
	Noseline,
	Glass,
	Body,
	Pants,
}

impl ModelPart {
	#[must_use]
	pub(super) const fn name(self) -> &'static str {
		match self {
			Self::Faceline => "OpaFaceline",
			Self::Beard => "OpaBeard",
			Self::Nose => "OpaNose",
			Self::Forehead => "OpaForehead",
			Self::Hair => "OpaHair",
			Self::Mask => "XluMask",
			Self::Noseline => "XluNoseLine",
			Self::Glass => "XluGlass",
			Self::Body => "Body",
			Self::Pants => "Pants",
		}
	}

	#[must_use]
	pub(super) const fn modulate_type(self) -> u8 {
		match self {
			Self::Faceline => 0,
			Self::Beard => 1,
			Self::Nose => 2,
			Self::Forehead => 3,
			Self::Hair => 4,
			Self::Mask => 6,
			Self::Noseline => 7,
			Self::Glass => 8,
			Self::Body => 9,
			Self::Pants => 10,
		}
	}

	pub(super) const fn is_translucent(self) -> bool {
		matches!(self, Self::Mask | Self::Noseline | Self::Glass)
	}
}

/// A renderer-ready RGBA texture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelTexture {
	pub(super) width: u16,
	pub(super) height: u16,
	pub(super) pixels: Vec<u8>,
}

impl ModelTexture {
	pub(super) fn from_resource(texture: &Texture) -> Self {
		let pixel_count = usize::from(texture.width) * usize::from(texture.height);
		let mut pixels = Vec::with_capacity(pixel_count * 4);
		match texture.format {
			TextureFormat::R8 => {
				for &red in &texture.pixels {
					pixels.extend_from_slice(&[red, red, red, u8::MAX]);
				}
			},
			TextureFormat::Rg8 => {
				for channels in texture.pixels.as_chunks::<2>().0 {
					pixels.extend_from_slice(&[channels[0], channels[1], 0, u8::MAX]);
				}
			},
			TextureFormat::Rgba8 => pixels.extend_from_slice(&texture.pixels),
		}
		Self {
			width: texture.width,
			height: texture.height,
			pixels,
		}
	}
}

/// Material colors and optional texture for one mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelMaterial {
	pub(super) mode: ModulateMode,
	pub(super) color: [f32; 4],
	pub(super) texture: Option<ModelTexture>,
}

/// One transformed mesh in FFL draw order.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelMesh {
	pub(super) part: ModelPart,
	pub(super) shape: Shape,
	pub(super) material: ModelMaterial,
}

/// Hair attachment transforms exposed by the reference GLB extras.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartsTransform {
	pub(super) front_translate: [f32; 3],
	pub(super) front_rotate: [f32; 3],
	pub(super) side_translate: [f32; 3],
	pub(super) side_rotate: [f32; 3],
	pub(super) top_translate: [f32; 3],
	pub(super) top_rotate: [f32; 3],
}

/// CPU character model shared by the raster and GLB backends.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterModel {
	pub(super) mii: Mii,
	pub(super) meshes: Vec<ModelMesh>,
	pub(super) parts_transform: Option<PartsTransform>,
}

impl CharacterModel {
	/// Builds the normal-head FFL model and composites its requested expression.
	///
	/// # Errors
	///
	/// Returns an error when a selected archive part cannot be decoded or the
	/// texture resolution is invalid.
	pub(super) fn build(
		archive: &ResourceArchive,
		mii: Mii,
		expression: Expression,
		texture_resolution: u16,
	) -> Result<Self> {
		crate::render::validate_texture_resolution(texture_resolution)?;
		Self::build_with_mask(archive, mii, expression, texture_resolution, None)
	}

	fn build_with_mask(
		archive: &ResourceArchive,
		mii: Mii,
		expression: Expression,
		texture_resolution: u16,
		mask_texture: Option<ModelTexture>,
	) -> Result<Self> {
		let mut faceline =
			archive.load_shape(ShapePart::Faceline, usize::from(mii.faceline_type))?;
		let ShapeTransform::Faceline { hair, nose, beard } = faceline.transform else {
			return Err(crate::Error::InvalidResource(
				"faceline shape is missing placement transforms".to_owned(),
			));
		};
		faceline.adjust(1.0, 1.0, [0.0; 3], false);

		let mut hair_shape =
			archive.load_shape(ShapePart::HairNormal, usize::from(mii.hair_type))?;
		let parts_transform = match hair_shape.transform {
			ShapeTransform::Hair {
				front_translate,
				front_rotate,
				side_translate,
				side_rotate,
				top_translate,
				top_rotate,
			} => Some(PartsTransform {
				front_translate,
				front_rotate,
				side_translate,
				side_rotate,
				top_translate,
				top_rotate,
			}),
			_ => None,
		};
		hair_shape.adjust(1.0, 1.0, hair, mii.hair_flip != 0);

		let mut forehead =
			archive.load_shape(ShapePart::ForeheadNormal, usize::from(mii.hair_type))?;
		forehead.adjust(1.0, 1.0, hair, mii.hair_flip != 0);

		let skin = color::faceline(mii.faceline_color);
		let hair_color = color::hair(mii.hair_color, mii.color_mode);
		let mut meshes = vec![ModelMesh {
			part: ModelPart::Faceline,
			shape: faceline,
			material: faceline_material(archive, &mii, texture_resolution, skin)?,
		}];

		add_shape_beard(archive, &mii, beard, &mut meshes)?;

		let blank_expression = matches!(expression, Expression::Afl61 | Expression::Afl62);
		let animal_expression = matches!(
			expression,
			Expression::Cat | Expression::CatDuplicate | Expression::Dog | Expression::DogDuplicate
		);
		let nose_placement = nose_placement(&mii, nose, blank_expression || animal_expression);
		if let Some((nose_scale, nose_position)) = nose_placement {
			let mut nose_shape = archive.load_shape(ShapePart::Nose, usize::from(mii.nose_type))?;
			nose_shape.adjust(nose_scale, nose_scale, nose_position, false);
			push_nonempty_mesh(&mut meshes, ModelPart::Nose, nose_shape, skin);
		}

		push_nonempty_mesh(&mut meshes, ModelPart::Forehead, forehead, skin);
		push_nonempty_mesh(&mut meshes, ModelPart::Hair, hair_shape, hair_color);

		if !blank_expression {
			let mut mask_shape =
				archive.load_shape(ShapePart::Mask, usize::from(mii.faceline_type))?;
			mask_shape.adjust(1.0, 1.0, [0.0; 3], false);
			let mask_texture = mask_texture.map_or_else(
				|| mask::compose(archive, &mii, expression, texture_resolution),
				Ok,
			)?;
			meshes.push(ModelMesh {
				part: ModelPart::Mask,
				shape: mask_shape,
				material: ModelMaterial {
					mode: ModulateMode::TextureDirect,
					color: [1.0; 4],
					texture: Some(mask_texture),
				},
			});
		}

		if let Some((nose_scale, nose_position)) = nose_placement {
			let mut noseline_shape =
				archive.load_shape(ShapePart::Noseline, usize::from(mii.nose_type))?;
			if !noseline_shape.indices.is_empty() {
				noseline_shape.adjust(nose_scale, nose_scale, nose_position, false);
				let noseline =
					archive.load_texture(TexturePart::Noseline, usize::from(mii.nose_type))?;
				meshes.push(ModelMesh {
					part: ModelPart::Noseline,
					shape: noseline_shape,
					material: ModelMaterial {
						mode: ModulateMode::Alpha,
						color: [0.0, 0.0, 0.0, 1.0],
						texture: Some(ModelTexture::from_resource(&noseline)),
					},
				});
			}
		}

		add_glasses(archive, &mii, nose, &mut meshes)?;

		Ok(Self {
			mii,
			meshes,
			parts_transform,
		})
	}
}

fn faceline_material(
	archive: &ResourceArchive,
	mii: &Mii,
	texture_resolution: u16,
	skin: [f32; 4],
) -> Result<ModelMaterial> {
	if faceline::is_enabled(mii) {
		Ok(ModelMaterial {
			mode: ModulateMode::TextureDirect,
			color: [1.0; 4],
			texture: Some(faceline::compose(archive, mii, texture_resolution)?),
		})
	} else {
		Ok(ModelMaterial {
			mode: ModulateMode::Constant,
			color: skin,
			texture: None,
		})
	}
}

fn add_shape_beard(
	archive: &ResourceArchive,
	mii: &Mii,
	position: [f32; 3],
	meshes: &mut Vec<ModelMesh>,
) -> Result<()> {
	if mii.beard_type < 4 {
		let mut shape = archive.load_shape(ShapePart::Beard, usize::from(mii.beard_type))?;
		if !shape.indices.is_empty() {
			shape.adjust(1.0, 1.0, position, false);
			meshes.push(mesh(
				ModelPart::Beard,
				shape,
				color::hair(mii.beard_color, mii.color_mode),
			));
		}
	}
	Ok(())
}

fn push_nonempty_mesh(meshes: &mut Vec<ModelMesh>, part: ModelPart, shape: Shape, color: [f32; 4]) {
	if !shape.indices.is_empty() {
		meshes.push(mesh(part, shape, color));
	}
}

fn nose_placement(mii: &Mii, nose: [f32; 3], hidden: bool) -> Option<(f32, [f32; 3])> {
	(!hidden).then(|| {
		let scale = f32::from(mii.nose_scale).mul_add(0.175, 0.4);
		let position = [
			nose[0],
			nose[1] + f32::from(i16::from(mii.nose_y) - 8) * -1.5,
			nose[2],
		];
		(scale, position)
	})
}

fn add_glasses(
	archive: &ResourceArchive,
	mii: &Mii,
	nose: [f32; 3],
	meshes: &mut Vec<ModelMesh>,
) -> Result<()> {
	if mii.glass_type == 0 {
		return Ok(());
	}
	let glass_scale = f32::from(mii.glass_scale).mul_add(0.175, 0.4);
	let glass_position = [
		nose[0],
		nose[1] + f32::from(i16::from(mii.glass_y) - 11).mul_add(-1.5, 5.0),
		nose[2] + 2.0,
	];
	let mut glass_shape = archive.load_shape(ShapePart::Glass, 0)?;
	glass_shape.adjust(glass_scale, glass_scale, glass_position, false);
	let glass = archive.load_texture(TexturePart::Glass, usize::from(mii.glass_type))?;
	meshes.push(ModelMesh {
		part: ModelPart::Glass,
		shape: glass_shape,
		material: ModelMaterial {
			mode: ModulateMode::LuminanceAlpha,
			color: color::glass(mii.glass_color, mii.color_mode),
			texture: Some(ModelTexture::from_resource(&glass)),
		},
	});
	Ok(())
}

const fn mesh(part: ModelPart, shape: Shape, color: [f32; 4]) -> ModelMesh {
	ModelMesh {
		part,
		shape,
		material: ModelMaterial {
			mode: ModulateMode::Constant,
			color,
			texture: None,
		},
	}
}
