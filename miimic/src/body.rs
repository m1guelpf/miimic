use std::{fs, path::Path};

use glam::{Mat4, Vec3, Vec4};
use num_traits::ToPrimitive as _;

use crate::{
	Error, Mii, ModelMaterial, ModelMesh, ModelPart, ModulateMode, Result, Shape, ShapeTransform,
	color, resource::bounds_of, resource_error, resource_source, usize_from_u32,
};

const BODY_MODEL_SCALE: f32 = 7.142_86;
const BONE_COUNT: usize = 25;
const HEAD_BONE: usize = 14;

#[derive(Clone, Debug)]
pub struct BodyAssets {
	male: RioModel,
	female: RioModel,
	skeleton: Vec<Bone>,
}

#[derive(Clone, Debug)]
struct RioModel {
	meshes: Vec<RioMesh>,
}

#[derive(Clone, Debug)]
struct RioMesh {
	vertices: Vec<RioVertex>,
	indices: Vec<u16>,
}

#[derive(Clone, Copy, Debug)]
struct RioVertex {
	position: Vec3,
	bone: usize,
	normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct Bone {
	parent: Option<usize>,
	local: Mat4,
}

pub struct BodyModel {
	pub(super) head_transform: Mat4,
	pub(super) head_translation: Vec3,
	pub(super) meshes: Vec<ModelMesh>,
}

impl BodyAssets {
	pub(super) fn near_resource(resource_path: &Path) -> Result<Option<Self>> {
		let Some(root) = resource_path.parent() else {
			return Ok(None);
		};
		let directory = root.join("fs/content/models/body");
		if !directory.is_dir() {
			return Ok(None);
		}
		Ok(Some(Self {
			male: RioModel::open(&directory.join("body_wiiu_male_LE.rmdl"))?,
			female: RioModel::open(&directory.join("body_wiiu_female_LE.rmdl"))?,
			skeleton: load_skeleton(&directory.join("body_wiiu_skeleton.csv"))?,
		}))
	}

	pub(super) fn build(&self, mii: &Mii) -> Result<BodyModel> {
		let body_scale = calculate_body_scale(mii);
		let (bones, head_transform) = calculate_bones(&self.skeleton, body_scale)?;
		let source = if mii.gender == 0 {
			&self.male
		} else {
			&self.female
		};
		let model_scale = Mat4::from_scale(Vec3::splat(BODY_MODEL_SCALE));
		let position_bones = bones
			.iter()
			.map(|bone| model_scale * *bone)
			.collect::<Vec<_>>();
		let colors = [
			color::favorite(mii.favorite_color),
			[0.250_980_4, 0.274_509_9, 0.305_882_4, 1.0],
		];
		let meshes = source
			.meshes
			.iter()
			.enumerate()
			.map(|(mesh_index, mesh)| {
				let mut positions = Vec::with_capacity(mesh.vertices.len());
				let mut normals = Vec::with_capacity(mesh.vertices.len());
				for vertex in &mesh.vertices {
					let (bone, position_bone) = bones
						.get(vertex.bone)
						.zip(position_bones.get(vertex.bone))
						.ok_or_else(|| {
							Error::InvalidResource(format!(
								"body vertex references missing bone {}",
								vertex.bone
							))
						})?;
					positions.push(position_bone.transform_point3(vertex.position).to_array());
					normals.push(
						bone.transform_vector3(vertex.normal)
							.normalize_or_zero()
							.to_array(),
					);
				}
				let bounds = bounds_of(&positions);
				let part = if mesh_index.is_multiple_of(2) {
					ModelPart::Body
				} else {
					ModelPart::Pants
				};
				Ok(ModelMesh {
					part,
					shape: Shape {
						positions,
						texcoords: Vec::new(),
						normals,
						tangents: Vec::new(),
						colors: vec![[u8::MAX, u8::MAX, 0, u8::MAX]],
						indices: mesh.indices.clone(),
						bounds,
						transform: ShapeTransform::None,
					},
					material: ModelMaterial {
						mode: ModulateMode::Constant,
						color: colors[usize::from(mesh_index % 2 != 0)],
						texture: None,
					},
				})
			})
			.collect::<Result<Vec<_>>>()?;
		let head_translation = head_transform.w_axis.truncate();
		Ok(BodyModel {
			head_transform,
			head_translation,
			meshes,
		})
	}
}

impl RioModel {
	fn open(path: &Path) -> Result<Self> {
		let bytes = fs::read(path)?;
		if bytes.get(..8) != Some(b"riomodel") {
			return Err(Error::InvalidResource(format!(
				"{} is not a RIO model",
				path.display()
			)));
		}
		if read_u32(&bytes, 8)? != 0x0100_0000 {
			return Err(Error::InvalidResource(format!(
				"{} has an unsupported RIO model version",
				path.display()
			)));
		}
		let mesh_offset = relative_offset(&bytes, 0x10)?;
		let mesh_count = usize_from_u32(read_u32(&bytes, 0x14)?)?;
		let mut meshes = Vec::with_capacity(mesh_count);
		for mesh_index in 0..mesh_count {
			let offset = mesh_offset
				.checked_add(mesh_index * 0x38)
				.ok_or_else(|| resource_error("body mesh offset overflow"))?;
			let vertex_offset = relative_offset(&bytes, offset)?;
			let vertex_count = usize_from_u32(read_u32(&bytes, offset + 4)?)?;
			let index_offset = relative_offset(&bytes, offset + 8)?;
			let index_count = usize_from_u32(read_u32(&bytes, offset + 12)?)?;
			let mut vertices = Vec::with_capacity(vertex_count);
			for vertex_index in 0..vertex_count {
				let vertex = vertex_offset
					.checked_add(vertex_index * 0x20)
					.ok_or_else(|| resource_error("body vertex offset overflow"))?;
				let bone = read_f32(&bytes, vertex + 12)?
					.to_usize()
					.ok_or_else(|| resource_error("body model has an invalid bone index"))?;
				vertices.push(RioVertex {
					position: Vec3::new(
						read_f32(&bytes, vertex)?,
						read_f32(&bytes, vertex + 4)?,
						read_f32(&bytes, vertex + 8)?,
					),
					bone,
					normal: Vec3::new(
						read_f32(&bytes, vertex + 20)?,
						read_f32(&bytes, vertex + 24)?,
						read_f32(&bytes, vertex + 28)?,
					),
				});
			}
			let mut indices = Vec::with_capacity(index_count);
			for index in 0..index_count {
				let value = read_u32(&bytes, index_offset + index * 4)?;
				indices.push(
					u16::try_from(value)
						.map_err(|_| resource_error("body model index does not fit in 16 bits"))?,
				);
			}
			meshes.push(RioMesh { vertices, indices });
		}
		Ok(Self { meshes })
	}
}

fn load_skeleton(path: &Path) -> Result<Vec<Bone>> {
	let source = fs::read_to_string(path)?;
	let mut bones = Vec::with_capacity(BONE_COUNT);
	for (line_index, line) in source.lines().skip(1).take(BONE_COUNT).enumerate() {
		let mut fields = line.trim_end_matches('\r').split(',');
		let bone_id = parse_i32(fields.next(), "bone ID")?;
		if bone_id != i32::try_from(line_index).expect("25 bones fit in i32") {
			return Err(resource_error("body skeleton bone IDs are not sequential"));
		}
		let parent_id = parse_i32(fields.next(), "parent bone ID")?;
		let mut matrix = [0.0_f32; 12];
		for value in &mut matrix {
			*value = fields
				.next()
				.ok_or_else(|| resource_error("body skeleton matrix is incomplete"))?
				.parse()
				.map_err(|error| {
					resource_source("body skeleton contains an invalid float", error)
				})?;
		}
		bones.push(Bone {
			parent: (parent_id >= 0)
				.then(|| usize::try_from(parent_id).expect("positive i32 fits usize")),
			local: matrix_from_rows(matrix),
		});
	}
	if bones.len() != BONE_COUNT {
		return Err(resource_error("body skeleton does not contain 25 bones"));
	}
	Ok(bones)
}

fn calculate_bones(source: &[Bone], body_scale: Vec3) -> Result<(Vec<Mat4>, Mat4)> {
	let mut matrices = source.iter().map(|bone| bone.local).collect::<Vec<_>>();
	for (bone_index, bone) in source.iter().enumerate() {
		let Some(parent) = bone.parent else {
			continue;
		};
		if parent >= bone_index {
			return Err(resource_error(
				"body skeleton parent must precede its child",
			));
		}
		let parent_scale = bone_scale(parent, body_scale)?;
		let mut translation = matrices[bone_index].w_axis.truncate();
		if bone_index == 2 {
			translation *= Vec3::new(body_scale.y, body_scale.y, body_scale.x);
			translation.y += body_scale.x - body_scale.y;
		}
		translation *= parent_scale;
		matrices[bone_index].w_axis = translation.extend(1.0);
		matrices[bone_index] = matrices[parent] * matrices[bone_index];
	}
	let mut head_transform = matrices[HEAD_BONE];
	head_transform.w_axis = (head_transform.w_axis.truncate() * BODY_MODEL_SCALE).extend(1.0);
	for (bone, matrix) in matrices.iter_mut().enumerate() {
		*matrix *= Mat4::from_scale(bone_scale(bone, body_scale)?);
	}
	Ok((matrices, head_transform))
}

fn bone_scale(bone: usize, body: Vec3) -> Result<Vec3> {
	let scale = match bone {
		0..=2 => Vec3::ONE,
		3 | 15..=18 | 21 | 22 => body,
		4 | 5 | 7 | 9 | 10 | 12 => Vec3::new(body.y, body.x, body.z),
		6 | 8 | 11 | 13 | 19 | 20 | 23 | 24 => Vec3::splat(body.x),
		14 => Vec3::new(body.x, body.y.max(1.0), body.z),
		_ => return Err(resource_error("body skeleton contains an unknown bone")),
	};
	Ok(scale)
}

fn calculate_body_scale(mii: &Mii) -> Vec3 {
	let build = f32::from(mii.build);
	let height = f32::from(mii.height);
	let x = build * height.mul_add(0.003_671_875, 0.4) / 128.0 + height.mul_add(0.001_796_875, 0.4);
	let y = height.mul_add(0.006_015_625, 0.5);
	Vec3::new(x, y, x)
}

const fn matrix_from_rows(m: [f32; 12]) -> Mat4 {
	Mat4::from_cols(
		Vec4::new(m[0], m[4], m[8], 0.0),
		Vec4::new(m[1], m[5], m[9], 0.0),
		Vec4::new(m[2], m[6], m[10], 0.0),
		Vec4::new(m[3], m[7], m[11], 1.0),
	)
}

fn relative_offset(bytes: &[u8], field: usize) -> Result<usize> {
	let offset = i64::from(read_i32(bytes, field)?);
	let absolute = i64::try_from(field)
		.map_err(|_| resource_error("offset field does not fit in i64"))?
		.checked_add(offset)
		.ok_or_else(|| resource_error("relative offset overflow"))?;
	usize::try_from(absolute).map_err(|_| resource_error("relative offset is negative"))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32> {
	let value = bytes
		.get(offset..offset + 4)
		.ok_or_else(|| resource_error("unexpected end of body resource"))?;
	Ok(i32::from_le_bytes(
		value.try_into().expect("four-byte slice"),
	))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
	let value = bytes
		.get(offset..offset + 4)
		.ok_or_else(|| resource_error("unexpected end of body resource"))?;
	Ok(u32::from_le_bytes(
		value.try_into().expect("four-byte slice"),
	))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32> {
	Ok(f32::from_bits(read_u32(bytes, offset)?))
}

fn parse_i32(value: Option<&str>, field: &'static str) -> Result<i32> {
	value
		.ok_or_else(|| resource_error(format!("body skeleton is missing {field}")))?
		.parse()
		.map_err(|error| resource_source(format!("body skeleton has an invalid {field}"), error))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::MiiData;

	const TEST_MII: &str = "0800600308040402020c0301060406020a0000000008000804000a2a003e7f04000214031304160d04000a040109";

	#[test]
	fn canonical_body_scale_matches_reference_formula() {
		let mii = Mii::from_data(&MiiData::decode(TEST_MII).expect("fixture should decode"))
			.expect("fixture should validate");
		let scale = calculate_body_scale(&mii);
		assert!((scale.x - 1.277_949_2).abs() < 0.000_001);
		assert!((scale.y - 1.263_984_4).abs() < 0.000_001);
		assert!((scale.x - scale.z).abs() < f32::EPSILON);
	}
}
