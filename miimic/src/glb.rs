use std::{borrow::Cow, collections::BTreeMap};

use gltf::{
	binary::{Glb, Header},
	json::{
		self, Index,
		accessor::{ComponentType, GenericComponentType, Type},
		buffer::Target,
		material::{AlphaMode, PbrBaseColorFactor},
		mesh::{Mode, Semantic},
		texture::{MagFilter, MinFilter, WrappingMode},
		validation::{Checked::Valid, USize64, Validate as _},
	},
};
use num_traits::ToPrimitive as _;
use serde_json::json;

use crate::{
	CharacterModel, Error, ModelMesh, ModulateMode, Result,
	body::BodyModel,
	composite::{encode_png, unorm_to_float},
	render_source,
};

/// Encodes a CPU character model as a self-contained glTF 2.0 binary file.
///
/// # Errors
///
/// Returns an error if its texture PNGs cannot be encoded, JSON serialization
/// fails, or the finished GLB exceeds the format's 32-bit size fields.
pub fn encode(model: &CharacterModel) -> Result<Vec<u8>> {
	encode_scene(model, None)
}

/// Encodes a CPU character model attached to a complete Wii U body.
///
/// # Errors
///
/// Returns an error if a mesh or texture cannot be encoded, JSON serialization
/// fails, or the finished GLB exceeds the format's 32-bit size fields.
pub fn encode_with_body(model: &CharacterModel, body: &BodyModel) -> Result<Vec<u8>> {
	encode_scene(model, Some(body))
}

fn encode_scene(model: &CharacterModel, body: Option<&BodyModel>) -> Result<Vec<u8>> {
	let mut builder = Builder::new();
	let mut nodes =
		Vec::with_capacity(model.meshes.len() + body.map_or(0, |body| body.meshes.len()));
	if let Some(body) = body {
		for mesh in &body.meshes {
			let mesh = builder.add_mesh(mesh)?;
			nodes.push(builder.root.push(json::Node {
				mesh: Some(mesh),
				..Default::default()
			}));
		}
	}

	let head_matrix = body.map(|body| body.head_transform.to_cols_array());
	for mesh in &model.meshes {
		let mesh = builder.add_mesh(mesh)?;
		nodes.push(builder.root.push(json::Node {
			matrix: head_matrix,
			mesh: Some(mesh),
			..Default::default()
		}));
	}
	let scene = builder.root.push(json::Scene {
		extensions: None,
		extras: None,
		name: None,
		nodes,
	});
	builder.root.scene = Some(scene);
	builder.root.asset.generator = Some("miimic".to_owned());
	builder.root.asset.extras = asset_extras(model)?;
	builder.root.buffers[builder.buffer.value()].byte_length = USize64::from(builder.binary.len());
	validate_document(&builder.root)?;

	let json_bytes = builder
		.root
		.to_vec()
		.map_err(|error| render_source("failed to serialize glTF JSON", error))?;
	let total_length = glb_length(json_bytes.len(), builder.binary.len())?;
	Glb {
		header: Header {
			magic: *b"glTF",
			version: 2,
			length: total_length,
		},
		json: Cow::Owned(json_bytes),
		bin: Some(Cow::Owned(builder.binary)),
	}
	.to_vec()
	.map_err(|error| render_source("failed to serialize binary glTF", error))
}

struct Builder {
	root: json::Root,
	buffer: Index<json::Buffer>,
	binary: Vec<u8>,
	non_mask_images: usize,
}

impl Builder {
	fn new() -> Self {
		let mut root = json::Root::default();
		let buffer = root.push(json::Buffer {
			byte_length: USize64(0),
			name: None,
			uri: None,
			extensions: None,
			extras: None,
		});
		Self {
			root,
			buffer,
			binary: Vec::new(),
			non_mask_images: 0,
		}
	}

	fn add_mesh(&mut self, mesh: &ModelMesh) -> Result<Index<json::Mesh>> {
		let vertex_count = mesh
			.shape
			.indices
			.iter()
			.copied()
			.max()
			.map_or(0, |index| usize::from(index) + 1);
		if vertex_count == 0 || vertex_count > mesh.shape.positions.len() {
			return Err(Error::Render(format!(
				"{} has no addressable vertices",
				mesh.part.name()
			)));
		}

		let attributes = self.add_attributes(mesh, vertex_count);
		let index_accessor = self.add_indices(mesh);
		let material_index = self.add_material(mesh)?;
		let mode = match mesh.material.mode {
			ModulateMode::Constant => 0,
			ModulateMode::TextureDirect | ModulateMode::Alpha | ModulateMode::LuminanceAlpha => 1,
		};
		let primitive = json::mesh::Primitive {
			attributes,
			extensions: None,
			extras: to_extras(&json!({
				"modulateColor": mesh.material.color,
				"modulateMode": mode,
				"modulateType": mesh.part.modulate_type(),
			}))?,
			indices: Some(index_accessor),
			material: Some(material_index),
			mode: Valid(Mode::Triangles),
			targets: None,
		};
		Ok(self.root.push(json::Mesh {
			extensions: None,
			extras: None,
			name: Some(mesh.part.name().to_owned()),
			primitives: vec![primitive],
			weights: None,
		}))
	}

	fn add_attributes(
		&mut self,
		mesh: &ModelMesh,
		vertex_count: usize,
	) -> BTreeMap<json::validation::Checked<Semantic>, Index<json::Accessor>> {
		let mut attributes = BTreeMap::new();
		let position_accessor = self.add_f32_attribute(
			&mesh.shape.positions[..vertex_count],
			Type::Vec3,
			Some(mesh.shape.bounds),
		);
		attributes.insert(Valid(Semantic::Positions), position_accessor);

		if mesh.material.texture.is_some() && mesh.shape.texcoords.len() >= vertex_count {
			let accessor =
				self.add_f32_attribute(&mesh.shape.texcoords[..vertex_count], Type::Vec2, None);
			attributes.insert(Valid(Semantic::TexCoords(0)), accessor);
		}

		if mesh.shape.normals.len() >= vertex_count {
			let accessor =
				self.add_f32_attribute(&mesh.shape.normals[..vertex_count], Type::Vec3, None);
			attributes.insert(Valid(Semantic::Normals), accessor);
		}

		if mesh.shape.tangents.len() >= vertex_count
			&& mesh
				.shape
				.tangents
				.first()
				.is_some_and(|tangent| tangent[3] != 0.0)
		{
			let accessor =
				self.add_f32_attribute(&mesh.shape.tangents[..vertex_count], Type::Vec4, None);
			attributes.insert(Valid(Semantic::Tangents), accessor);
		}

		let mut colors = Vec::with_capacity(vertex_count * 4);
		if mesh.shape.colors.len() >= vertex_count {
			colors.extend(mesh.shape.colors[..vertex_count].iter().flatten());
		} else if let Some(color) = mesh.shape.colors.first() {
			for _ in 0..vertex_count {
				colors.extend_from_slice(color);
			}
		} else {
			colors.resize(vertex_count * 4, 0);
		}
		let color_accessor = self.add_bytes(
			&colors,
			Target::ArrayBuffer,
			ComponentType::U8,
			vertex_count,
			Type::Vec4,
			true,
		);
		attributes.insert(Valid(Semantic::Extras("COLOR".to_owned())), color_accessor);
		attributes
	}

	fn add_indices(&mut self, mesh: &ModelMesh) -> Index<json::Accessor> {
		let mut index_bytes = Vec::with_capacity(mesh.shape.indices.len() * 2);
		for index in &mesh.shape.indices {
			index_bytes.extend_from_slice(&index.to_le_bytes());
		}
		self.add_bytes(
			&index_bytes,
			Target::ElementArrayBuffer,
			ComponentType::U16,
			mesh.shape.indices.len(),
			Type::Scalar,
			false,
		)
	}

	fn add_f32_attribute<const N: usize>(
		&mut self,
		values: &[[f32; N]],
		kind: Type,
		bounds: Option<[[f32; 3]; 2]>,
	) -> Index<json::Accessor> {
		let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
		for value in values {
			for component in value {
				bytes.extend_from_slice(&component.to_le_bytes());
			}
		}
		let index = self.add_bytes(
			&bytes,
			Target::ArrayBuffer,
			ComponentType::F32,
			values.len(),
			kind,
			false,
		);
		if let Some([min, max]) = bounds {
			let accessor = &mut self.root.accessors[index.value()];
			accessor.min = Some(json::Value::from(Vec::from(min)));
			accessor.max = Some(json::Value::from(Vec::from(max)));
		}
		index
	}

	fn add_bytes(
		&mut self,
		bytes: &[u8],
		target: Target,
		component_type: ComponentType,
		count: usize,
		kind: Type,
		normalized: bool,
	) -> Index<json::Accessor> {
		pad_to(&mut self.binary, component_type.size(), 0);
		let offset = self.binary.len();
		self.binary.extend_from_slice(bytes);
		let view = self.root.push(json::buffer::View {
			buffer: self.buffer,
			byte_length: USize64::from(bytes.len()),
			byte_offset: Some(USize64::from(offset)),
			byte_stride: None,
			name: None,
			target: Some(Valid(target)),
			extensions: None,
			extras: None,
		});
		self.root.push(json::Accessor {
			buffer_view: Some(view),
			byte_offset: None,
			count: USize64::from(count),
			component_type: Valid(GenericComponentType(component_type)),
			extensions: None,
			extras: None,
			type_: Valid(kind),
			min: None,
			max: None,
			name: None,
			normalized,
			sparse: None,
		})
	}

	fn add_material(&mut self, mesh: &ModelMesh) -> Result<Index<json::Material>> {
		let material_name = if mesh.part == crate::ModelPart::Mask {
			format!("Material_{}_0", mesh.part.name())
		} else {
			format!("Material_{}", mesh.part.name())
		};
		let mut material = json::Material {
			name: Some(material_name),
			..Default::default()
		};
		if let Some(texture) = &mesh.material.texture {
			let pixels = apply_texture_modulation(mesh);
			let png = encode_png(u32::from(texture.width), u32::from(texture.height), &pixels)?;
			pad_to(&mut self.binary, 4, 0);
			let offset = self.binary.len();
			self.binary.extend_from_slice(&png);
			let view = self.root.push(json::buffer::View {
				buffer: self.buffer,
				byte_length: USize64::from(png.len()),
				byte_offset: Some(USize64::from(offset)),
				byte_stride: None,
				name: None,
				target: None,
				extensions: None,
				extras: None,
			});
			let image_name = if mesh.part == crate::ModelPart::Mask {
				"MaskTexture_0".to_owned()
			} else {
				let name = format!("Texture_{}", self.non_mask_images);
				self.non_mask_images += 1;
				name
			};
			let image = self.root.push(json::Image {
				buffer_view: Some(view),
				mime_type: Some(json::image::MimeType("image/png".to_owned())),
				name: Some(image_name),
				uri: None,
				extensions: None,
				extras: None,
			});
			let sampler = self.root.push(json::texture::Sampler {
				mag_filter: Some(Valid(MagFilter::Linear)),
				min_filter: Some(Valid(MinFilter::LinearMipmapLinear)),
				wrap_s: Valid(WrappingMode::MirroredRepeat),
				wrap_t: Valid(WrappingMode::MirroredRepeat),
				..Default::default()
			});
			let texture_index = self.root.push(json::Texture {
				name: None,
				sampler: Some(sampler),
				source: image,
				extensions: None,
				extras: None,
			});
			material.alpha_mode = Valid(match mesh.part {
				crate::ModelPart::Mask => AlphaMode::Mask,
				crate::ModelPart::Noseline | crate::ModelPart::Glass => AlphaMode::Blend,
				_ => AlphaMode::Opaque,
			});
			material.pbr_metallic_roughness.base_color_texture = Some(json::texture::Info {
				index: texture_index,
				tex_coord: 0,
				extensions: None,
				extras: None,
			});
		} else {
			material.pbr_metallic_roughness.base_color_factor =
				PbrBaseColorFactor(mesh.material.color.map(srgb_to_linear));
		}
		Ok(self.root.push(material))
	}
}

fn asset_extras(model: &CharacterModel) -> Result<json::Extras> {
	let mut extras = json!({
		"additionalInfo": {
			"build": model.mii.build,
			"facelineType": model.mii.faceline_type,
			"favoriteColor": model.mii.favorite_color,
			"gender": model.mii.gender,
			"hairFlip": model.mii.hair_flip,
			"hairType": model.mii.hair_type,
			"height": model.mii.height,
			"skinColor": crate::color::faceline(model.mii.faceline_color),
		}
	});
	if let Some(transform) = model.parts_transform {
		extras["partsTransform"] = json!({
			"hatTranslate": [0.0, 0.0, 0.0],
			"headFrontRotate": transform.front_rotate,
			"headFrontTranslate": transform.front_translate,
			"headSideRotate": transform.side_rotate,
			"headSideTranslate": transform.side_translate,
			"headTopRotate": transform.top_rotate,
			"headTopTranslate": transform.top_translate,
		});
	}
	to_extras(&extras)
}

fn to_extras(value: &serde_json::Value) -> Result<json::Extras> {
	serde_json::value::to_raw_value(value)
		.map(Some)
		.map_err(|error| render_source("failed to serialize glTF extras", error))
}

fn validate_document(root: &json::Root) -> Result<()> {
	let mut errors = Vec::new();
	root.validate(root, json::Path::new, &mut |path, error| {
		errors.push((path(), error));
	});
	if errors.is_empty() {
		return Ok(());
	}
	let details = errors
		.into_iter()
		.map(|(path, error)| format!("{path}: {error:?}"))
		.collect::<Vec<_>>()
		.join(", ");
	Err(Error::Render(format!(
		"generated invalid glTF document: {details}"
	)))
}

fn glb_length(json_length: usize, binary_length: usize) -> Result<u32> {
	let json_length = padded_length(json_length)?;
	let binary_length = padded_length(binary_length)?;
	let total_length = 12_usize
		.checked_add(8)
		.and_then(|length| length.checked_add(json_length))
		.and_then(|length| length.checked_add(8))
		.and_then(|length| length.checked_add(binary_length))
		.ok_or_else(|| Error::Render("GLB length overflow".to_owned()))?;
	u32::try_from(total_length)
		.map_err(|_| Error::Render("GLB exceeds the 32-bit format limit".to_owned()))
}

fn padded_length(length: usize) -> Result<usize> {
	length
		.checked_add(3)
		.map(|length| length & !3)
		.ok_or_else(|| Error::Render("GLB length overflow".to_owned()))
}

fn apply_texture_modulation(mesh: &ModelMesh) -> Vec<u8> {
	let texture = mesh
		.material
		.texture
		.as_ref()
		.expect("texture modulation is only called for textured materials");
	if mesh.material.mode == ModulateMode::TextureDirect {
		return texture.pixels.clone();
	}
	let mut output = Vec::with_capacity(texture.pixels.len());
	for pixel in texture.pixels.as_chunks::<4>().0 {
		let red = unorm_to_float(pixel[0]);
		let intensity = if mesh.material.mode == ModulateMode::LuminanceAlpha {
			unorm_to_float(pixel[1])
		} else {
			red
		};
		let channel = |color: f32| {
			(color * intensity * 255.0)
				.to_u8()
				.expect("modulated texture channel fits u8")
		};
		output.extend_from_slice(&[
			channel(mesh.material.color[0]),
			channel(mesh.material.color[1]),
			channel(mesh.material.color[2]),
			(red * 255.0)
				.to_u8()
				.expect("texture alpha channel fits u8"),
		]);
	}
	output
}

fn srgb_to_linear(color: f32) -> f32 {
	if color <= 0.040_45 {
		color / 12.92
	} else {
		((color + 0.055) / 1.055).powf(2.4)
	}
}

fn pad_to(bytes: &mut Vec<u8>, alignment: usize, value: u8) {
	let padding = (alignment - bytes.len() % alignment) % alignment;
	bytes.resize(bytes.len() + padding, value);
}

#[cfg(test)]
mod tests {
	use super::glb_length;

	#[test]
	fn glb_length_includes_padded_chunks() {
		assert_eq!(glb_length(1, 1).expect("small GLB should fit"), 36);
	}

	#[test]
	fn glb_length_rejects_format_overflow() {
		let error = glb_length(u32::MAX as usize, 0).expect_err("oversized GLB should fail");
		assert_eq!(
			error.to_string(),
			"rendering failed: GLB exceeds the 32-bit format limit"
		);
	}
}
