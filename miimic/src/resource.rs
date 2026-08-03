use std::{
	collections::HashMap,
	fs,
	io::Read as _,
	path::{Path, PathBuf},
	sync::{Arc, Mutex, PoisonError},
};

use flate2::read::ZlibDecoder;
use sha2::{Digest as _, Sha256};

use crate::{Error, Result, resource_error, resource_source, usize_from_u32};

const MAGIC: &[u8; 4] = b"FFRA";
const VERSION: u32 = 0x0007_0000;
const BASE_HEADER_SIZE: usize = 20;
const PART_INFO_SIZE: usize = 16;
const AFL_2_3_HIGH_EXPANDED_SIZE: u32 = 0x0250_2de0;
const AFL_2_3_HIGH_TEXTURE_HEADER_SIZE: usize = 0x16fc;
const AFL_2_3_HIGH_HEADER_SIZE: usize = 0x4d00;
const MAX_EXPANDED_ENTRY_SIZE: usize = 1024 * 1024;

const TRUSTED_ARCHIVE_HASHES: [[u8; 32]; 1] = [[
	0x4a, 0x4b, 0xe7, 0x1d, 0x75, 0x16, 0x2c, 0x20, 0xb4, 0x87, 0x20, 0xef, 0x89, 0xcd, 0x3d, 0x4e,
	0x6c, 0xd4, 0xe8, 0xe2, 0x1b, 0xcb, 0xc2, 0xae, 0xd6, 0x3e, 0xe0, 0x8d, 0x96, 0xde, 0x77, 0x22,
]];

const AFL_2_3_TEXTURE_COUNTS: [usize; 11] = [3, 132, 80, 28, 12, 12, 20, 2, 52, 6, 18];
const SHAPE_COUNTS: [usize; 12] = [4, 132, 132, 12, 1, 12, 18, 18, 132, 132, 132, 132];

/// Kind of archive entry required by model construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
	Shape,
	Texture,
}

impl std::fmt::Display for ResourceKind {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str(match self {
			Self::Shape => "shape",
			Self::Texture => "texture",
		})
	}
}

/// A resource texture category in the order stored in `FFLRes*.dat`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum TexturePart {
	Beard = 0,
	Eye = 2,
	Eyebrow = 3,
	Faceline = 4,
	FaceMakeup = 5,
	Glass = 6,
	Mole = 7,
	Mouth = 8,
	Mustache = 9,
	Noseline = 10,
}

impl TexturePart {
	const fn index(self) -> usize {
		self as usize
	}

	const fn name(self) -> &'static str {
		match self {
			Self::Beard => "beard",
			Self::Eye => "eye",
			Self::Eyebrow => "eyebrow",
			Self::Faceline => "faceline",
			Self::FaceMakeup => "face_makeup",
			Self::Glass => "glass",
			Self::Mole => "mole",
			Self::Mouth => "mouth",
			Self::Mustache => "mustache",
			Self::Noseline => "noseline",
		}
	}
}

/// A resource mesh category in the order stored in `FFLRes*.dat`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum ShapePart {
	Beard = 0,
	Faceline = 3,
	Glass = 4,
	Mask = 5,
	Noseline = 6,
	Nose = 7,
	HairNormal = 8,
	ForeheadNormal = 10,
}

impl ShapePart {
	const fn index(self) -> usize {
		self as usize
	}

	const fn name(self) -> &'static str {
		match self {
			Self::Beard => "beard",
			Self::Faceline => "faceline",
			Self::Glass => "glass",
			Self::Mask => "mask",
			Self::Noseline => "noseline",
			Self::Nose => "nose",
			Self::HairNormal => "hair_normal",
			Self::ForeheadNormal => "forehead_normal",
		}
	}
}

/// Pixel layout used by an FFL texture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureFormat {
	R8,
	Rg8,
	Rgba8,
}

impl TextureFormat {
	const fn bytes_per_pixel(self) -> usize {
		match self {
			Self::R8 => 1,
			Self::Rg8 => 2,
			Self::Rgba8 => 4,
		}
	}
}

/// A decoded, linear base-level texture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Texture {
	pub(super) width: u16,
	pub(super) height: u16,
	pub(super) format: TextureFormat,
	pub(super) pixels: Vec<u8>,
}

/// Optional placement metadata embedded in faceline and hair meshes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeTransform {
	None,
	Faceline {
		hair: [f32; 3],
		nose: [f32; 3],
		beard: [f32; 3],
	},
	Hair {
		front_translate: [f32; 3],
		front_rotate: [f32; 3],
		side_translate: [f32; 3],
		side_rotate: [f32; 3],
		top_translate: [f32; 3],
		top_rotate: [f32; 3],
	},
}

/// A decoded FFL mesh using renderer-ready vertex formats.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
	pub(super) positions: Vec<[f32; 3]>,
	pub(super) texcoords: Vec<[f32; 2]>,
	pub(super) normals: Vec<[f32; 3]>,
	pub(super) tangents: Vec<[f32; 4]>,
	pub(super) colors: Vec<[u8; 4]>,
	pub(super) indices: Vec<u16>,
	pub(super) bounds: [[f32; 3]; 2],
	pub(super) transform: ShapeTransform,
}

impl Shape {
	/// Applies the same in-place scale, translation, and horizontal flip that
	/// `FFLiAdjustShape` performs with the default coordinate system.
	pub(super) fn adjust(&mut self, scale_x: f32, scale_y: f32, translate: [f32; 3], flip_x: bool) {
		let x_scale = if flip_x { -scale_x } else { scale_x };
		let z_scale = f32::midpoint(scale_x, scale_y);
		for position in &mut self.positions {
			position[0] = position[0].mul_add(x_scale, translate[0]);
			position[1] = position[1].mul_add(scale_y, translate[1]);
			position[2] = position[2].mul_add(z_scale, translate[2]);
		}
		if flip_x {
			for normal in &mut self.normals {
				normal[0] = -normal[0];
			}
			for tangent in &mut self.tangents {
				tangent[0] = -tangent[0];
				tangent[3] = -tangent[3];
			}
			for triangle in self.indices.as_chunks_mut::<3>().0 {
				triangle.swap(1, 2);
			}
		}
		self.recalculate_bounds();
	}

	fn recalculate_bounds(&mut self) {
		self.bounds = bounds_of(&self.positions);
	}
}

pub fn bounds_of(positions: &[[f32; 3]]) -> [[f32; 3]; 2] {
	let Some(first) = positions.first().copied() else {
		return [[f32::NAN; 3]; 2];
	};
	let mut min = first;
	let mut max = first;
	for position in &positions[1..] {
		for axis in 0..3 {
			min[axis] = min[axis].min(position[axis]);
			max[axis] = max[axis].max(position[axis]);
		}
	}
	[min, max]
}

/// Immutable reader for an AFL 2.3 high resource archive.
#[derive(Clone)]
pub struct ResourceArchive {
	bytes: Arc<[u8]>,
	expanded_entries: Arc<Mutex<HashMap<PartInfo, Arc<[u8]>>>>,
	source_path: Option<PathBuf>,
}

impl std::fmt::Debug for ResourceArchive {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("ResourceArchive")
			.field("byte_len", &self.bytes.len())
			.finish_non_exhaustive()
	}
}

fn validate_layout(bytes: &[u8]) -> Result<()> {
	let expanded_size = read_u32(bytes, 12)?;
	let hint = expanded_size >> 29;
	let untagged_size = expanded_size & 0x1fff_ffff;
	if !matches!(hint, 0 | 3) || untagged_size != AFL_2_3_HIGH_EXPANDED_SIZE {
		return Err(resource_error(
			"only AFL 2.3 high resource archives are supported",
		));
	}
	Ok(())
}

impl ResourceArchive {
	/// Opens a trusted AFL 2.3 high resource archive.
	///
	/// # Errors
	///
	/// Returns an I/O error when the file cannot be read and
	/// [`Error::InvalidResource`] when its header is unsupported or malformed.
	/// Returns [`Error::UntrustedResourceArchive`] when the archive is structurally
	/// supported but its SHA-256 digest is not in Miimic's trusted archive list.
	pub fn open(path: impl AsRef<Path>) -> Result<Self> {
		let path = path.as_ref();
		let mut archive = Self::from_bytes(fs::read(path)?)?;
		archive.source_path = Some(path.to_owned());
		Ok(archive)
	}

	pub(super) fn source_path(&self) -> Option<&Path> {
		self.source_path.as_deref()
	}

	/// Validates and verifies an in-memory AFL 2.3 high resource archive.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidResource`] when the header is unsupported or malformed.
	/// Returns [`Error::UntrustedResourceArchive`] when the archive is structurally
	/// supported but its SHA-256 digest is not in Miimic's trusted archive list.
	fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
		let archive = Self::unchecked_from_bytes(bytes)?;
		verify_trusted_archive(&archive.bytes)?;
		Ok(archive)
	}

	/// Validates an in-memory AFL 2.3 high resource archive without requiring a
	/// trusted digest.
	///
	/// This bypasses only trusted-archive certification. The archive's FFRA header,
	/// version, and AFL 2.3 high layout are still validated.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidResource`] when the header is unsupported or
	/// malformed.
	fn unchecked_from_bytes(bytes: Vec<u8>) -> Result<Self> {
		if bytes.len() < BASE_HEADER_SIZE {
			return Err(resource_error("file is shorter than its base header"));
		}
		if bytes.get(..4) != Some(MAGIC) {
			return Err(resource_error("invalid FFRA magic"));
		}
		if read_u32(&bytes, 4)? != VERSION {
			return Err(resource_error("unsupported FFL resource version"));
		}
		if read_u32(&bytes, 16)? == 1 {
			return Err(resource_error("expanded resource archives are unsupported"));
		}
		validate_layout(&bytes)?;
		if bytes.len() < AFL_2_3_HIGH_HEADER_SIZE {
			return Err(resource_error(format!(
				"file is shorter than its 0x{AFL_2_3_HIGH_HEADER_SIZE:x}-byte header"
			)));
		}
		Ok(Self {
			bytes: bytes.into(),
			expanded_entries: Arc::new(Mutex::new(HashMap::new())),
			source_path: None,
		})
	}

	#[cfg(test)]
	#[must_use]
	fn len(&self) -> usize {
		self.bytes.len()
	}

	/// Loads and decodes a mesh resource.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidResource`] for an invalid part index, malformed
	/// archive entry, compressed stream, or mesh payload.
	pub(super) fn load_shape(&self, part: ShapePart, index: usize) -> Result<Shape> {
		let entry = self.shape_entry(part, index)?;
		Shape::decode(&self.load_entry(entry)?, part)
	}

	/// Loads the linear base mip level of a texture resource.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidResource`] for an invalid part index, malformed
	/// archive entry, compressed stream, or texture footer.
	pub(super) fn load_texture(&self, part: TexturePart, index: usize) -> Result<Texture> {
		let entry = self.texture_entry(part, index)?;
		Texture::decode(&self.load_entry(entry)?)
	}

	pub(super) const fn require_shape(part: ShapePart, index: usize) -> Result<()> {
		require_entry(
			ResourceKind::Shape,
			part.name(),
			index,
			SHAPE_COUNTS[part.index()],
		)
	}

	pub(super) const fn require_texture(part: TexturePart, index: usize) -> Result<()> {
		require_entry(
			ResourceKind::Texture,
			part.name(),
			index,
			AFL_2_3_TEXTURE_COUNTS[part.index()],
		)
	}

	fn texture_entry(&self, part: TexturePart, index: usize) -> Result<PartInfo> {
		Self::require_texture(part, index)?;
		entry_at(
			&self.bytes,
			BASE_HEADER_SIZE + AFL_2_3_TEXTURE_COUNTS.len() * 4,
			&AFL_2_3_TEXTURE_COUNTS,
			part.index(),
			index,
		)
	}

	fn shape_entry(&self, part: ShapePart, index: usize) -> Result<PartInfo> {
		Self::require_shape(part, index)?;
		entry_at(
			&self.bytes,
			BASE_HEADER_SIZE + AFL_2_3_HIGH_TEXTURE_HEADER_SIZE + SHAPE_COUNTS.len() * 4,
			&SHAPE_COUNTS,
			part.index(),
			index,
		)
	}

	fn load_entry(&self, entry: PartInfo) -> Result<Arc<[u8]>> {
		if entry.data_size > MAX_EXPANDED_ENTRY_SIZE {
			return Err(resource_error(format!(
				"expanded entry has {} bytes, maximum is {MAX_EXPANDED_ENTRY_SIZE}",
				entry.data_size
			)));
		}
		if entry.data_size == 0 {
			return Ok(Arc::from([]));
		}
		let cached = {
			let entries = self
				.expanded_entries
				.lock()
				.unwrap_or_else(PoisonError::into_inner);
			entries.get(&entry).map(Arc::clone)
		};
		if let Some(expanded) = cached {
			return Ok(expanded);
		}
		let stored_size = if entry.strategy == 5 {
			entry.data_size
		} else {
			entry.compressed_size
		};
		let stored = checked_slice(&self.bytes, entry.data_pos, stored_size)?;
		let expanded: Arc<[u8]> = if entry.strategy == 5 {
			Arc::from(stored)
		} else {
			if entry.strategy > 4 {
				return Err(resource_error(format!(
					"unsupported compression strategy {}",
					entry.strategy
				)));
			}

			let output_limit = entry
				.data_size
				.checked_add(1)
				.ok_or_else(|| resource_error("expanded entry limit overflow"))?;
			let decoder = ZlibDecoder::new(stored);
			let mut decoder = decoder.take(
				u64::try_from(output_limit)
					.map_err(|_| resource_error("expanded entry limit does not fit u64"))?,
			);
			let mut expanded = Vec::with_capacity(output_limit);
			decoder
				.read_to_end(&mut expanded)
				.map_err(|error| resource_source("zlib decompression failed", error))?;
			if expanded.len() != entry.data_size {
				return Err(resource_error(format!(
					"decoded entry has {} bytes, expected {}",
					expanded.len(),
					entry.data_size
				)));
			}
			expanded.into()
		};

		let mut entries = self
			.expanded_entries
			.lock()
			.unwrap_or_else(PoisonError::into_inner);
		Ok(Arc::clone(entries.entry(entry).or_insert(expanded)))
	}
}

fn verify_trusted_archive(bytes: &[u8]) -> Result<()> {
	let sha256: [u8; 32] = Sha256::digest(bytes).into();
	if !TRUSTED_ARCHIVE_HASHES.contains(&sha256) {
		return Err(Error::UntrustedResourceArchive {
			sha256: hex::encode(sha256),
		});
	}
	Ok(())
}

const fn require_entry(
	kind: ResourceKind,
	part: &'static str,
	index: usize,
	available: usize,
) -> Result<()> {
	if index >= available {
		return Err(Error::UnsupportedResource {
			kind,
			part,
			index,
			available,
		});
	}
	Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PartInfo {
	data_pos: usize,
	data_size: usize,
	compressed_size: usize,
	strategy: u8,
}

fn entry_at(
	bytes: &[u8],
	entries_offset: usize,
	counts: &[usize],
	part: usize,
	index: usize,
) -> Result<PartInfo> {
	let count = *counts
		.get(part)
		.ok_or_else(|| resource_error("invalid resource part type"))?;
	if index >= count {
		return Err(resource_error(format!(
			"part index {index} is outside 0..{count}"
		)));
	}
	let preceding = counts[..part].iter().sum::<usize>();
	let offset = entries_offset
		.checked_add((preceding + index) * PART_INFO_SIZE)
		.ok_or_else(|| resource_error("part table offset overflow"))?;
	let info = checked_slice(bytes, offset, PART_INFO_SIZE)?;
	Ok(PartInfo {
		data_pos: usize_from_u32(read_u32(info, 0)?)?,
		data_size: usize_from_u32(read_u32(info, 4)?)?,
		compressed_size: usize_from_u32(read_u32(info, 8)?)?,
		strategy: info[15],
	})
}

impl Shape {
	fn decode(data: &[u8], part: ShapePart) -> Result<Self> {
		if data.is_empty() {
			return Ok(Self {
				positions: Vec::new(),
				texcoords: Vec::new(),
				normals: Vec::new(),
				tangents: Vec::new(),
				colors: Vec::new(),
				indices: Vec::new(),
				bounds: [[f32::NAN; 3]; 2],
				transform: ShapeTransform::None,
			});
		}
		if data.len() < 0x90 {
			return Err(resource_error("shape is shorter than its 0x90-byte header"));
		}

		let mut positions = [0_usize; 6];
		let mut sizes = [0_usize; 6];
		for element in 0..6 {
			positions[element] = usize_from_u32(read_u32(data, element * 4)?)?;
			sizes[element] = usize_from_u32(read_u32(data, 24 + element * 4)?)?;
		}

		let position_data = element_bytes(data, positions[0], sizes[0])?;
		if !position_data.len().is_multiple_of(16) {
			return Err(resource_error("position element is not Vec4-aligned"));
		}
		let mut decoded_positions = Vec::with_capacity(position_data.len() / 16);
		for vector in position_data.as_chunks::<16>().0 {
			decoded_positions.push([
				read_f32(vector, 0)?,
				read_f32(vector, 4)?,
				read_f32(vector, 8)?,
			]);
		}

		let normal_data = element_bytes(data, positions[1], sizes[1])?;
		if !normal_data.len().is_multiple_of(4) {
			return Err(resource_error("normal element is not u32-aligned"));
		}
		let mut normals = Vec::with_capacity(normal_data.len() / 4);
		for packed in normal_data.as_chunks::<4>().0 {
			normals.push(unpack_normal(read_u32(packed, 0)?));
		}

		let texcoord_data = element_bytes(data, positions[2], sizes[2])?;
		if !texcoord_data.len().is_multiple_of(8) {
			return Err(resource_error("texcoord element is not Vec2-aligned"));
		}
		let mut texcoords = Vec::with_capacity(texcoord_data.len() / 8);
		for vector in texcoord_data.as_chunks::<8>().0 {
			texcoords.push([read_f32(vector, 0)?, read_f32(vector, 4)?]);
		}

		let tangent_data = element_bytes(data, positions[3], sizes[3])?;
		if !tangent_data.len().is_multiple_of(4) {
			return Err(resource_error("tangent element is not four-byte aligned"));
		}
		let tangents = tangent_data
			.as_chunks::<4>()
			.0
			.iter()
			.map(|tangent| decode_tangent(tangent))
			.collect();

		let color_data = element_bytes(data, positions[4], sizes[4])?;
		if !color_data.len().is_multiple_of(4) {
			return Err(resource_error("color element is not four-byte aligned"));
		}
		let colors = color_data
			.as_chunks::<4>()
			.0
			.iter()
			.map(|color| [color[0], color[1], color[2], color[3]])
			.collect();

		let indices = decode_indices(data, positions[5], sizes[5])?;

		let bounds = [read_vec3(data, 48)?, read_vec3(data, 60)?];
		let transform = match part {
			ShapePart::Faceline => ShapeTransform::Faceline {
				hair: read_vec3(data, 72)?,
				nose: read_vec3(data, 84)?,
				beard: read_vec3(data, 96)?,
			},
			ShapePart::HairNormal => ShapeTransform::Hair {
				front_translate: read_vec3(data, 72)?,
				front_rotate: read_vec3(data, 84)?,
				side_translate: read_vec3(data, 96)?,
				side_rotate: read_vec3(data, 108)?,
				top_translate: read_vec3(data, 120)?,
				top_rotate: read_vec3(data, 132)?,
			},
			_ => ShapeTransform::None,
		};

		Ok(Self {
			positions: decoded_positions,
			texcoords,
			normals,
			tangents,
			colors,
			indices,
			bounds,
			transform,
		})
	}
}

fn decode_indices(data: &[u8], position: usize, count: usize) -> Result<Vec<u16>> {
	let byte_size = count
		.checked_mul(2)
		.ok_or_else(|| resource_error("index byte size overflow"))?;
	let bytes = element_bytes(data, position, byte_size)?;
	Ok(bytes
		.as_chunks::<2>()
		.0
		.iter()
		.map(|index| u16::from_be_bytes(*index))
		.collect())
}

impl Texture {
	fn decode(data: &[u8]) -> Result<Self> {
		if data.len() < 12 {
			return Err(resource_error("texture is shorter than its 12-byte footer"));
		}
		let footer = data.len() - 12;
		let width = read_u16(data, footer + 4)?;
		let height = read_u16(data, footer + 6)?;
		if width == 0 || height == 0 {
			return Err(resource_error(format!(
				"texture dimensions must be nonzero, got {width}x{height}"
			)));
		}
		let format = match data[footer + 9] {
			0 => TextureFormat::R8,
			1 => TextureFormat::Rg8,
			2 => TextureFormat::Rgba8,
			value => {
				return Err(resource_error(format!(
					"unsupported texture format {value}"
				)));
			},
		};
		let expected = usize::from(width)
			.checked_mul(usize::from(height))
			.and_then(|pixels| pixels.checked_mul(format.bytes_per_pixel()))
			.ok_or_else(|| resource_error("texture dimensions overflow"))?;
		let pixels = checked_slice(data, 0, expected)?.to_vec();
		Ok(Self {
			width,
			height,
			format,
			pixels,
		})
	}
}

fn unpack_normal(packed: u32) -> [f32; 3] {
	let x = sign_extend_10(packed);
	let y = sign_extend_10(packed >> 10);
	let z = sign_extend_10(packed >> 20);
	[
		f32::from(x) / 511.0,
		f32::from(y) / 511.0,
		f32::from(z) / 511.0,
	]
}

fn sign_extend_10(value: u32) -> i16 {
	let value = u16::try_from(value & 0x3ff).expect("ten bits always fit in u16");
	if value & 0x200 == 0 {
		i16::try_from(value).expect("positive ten-bit value fits in i16")
	} else {
		i16::from_be_bytes((value | 0xfc00).to_be_bytes())
	}
}

fn decode_tangent(bytes: &[u8]) -> [f32; 4] {
	let signed = [
		i8::from_ne_bytes([bytes[0]]),
		i8::from_ne_bytes([bytes[1]]),
		i8::from_ne_bytes([bytes[2]]),
		i8::from_ne_bytes([bytes[3]]),
	];
	if signed[3] == 0 {
		return [0.0; 4];
	}
	let mut result = [
		f32::from(signed[0]) / 127.0,
		f32::from(signed[1]) / 127.0,
		f32::from(signed[2]) / 127.0,
		f32::from(signed[3]) / 127.0,
	];
	let magnitude = result[..3]
		.iter()
		.map(|component| component * component)
		.sum::<f32>()
		.sqrt();
	if magnitude != 0.0 {
		for component in &mut result[..3] {
			*component /= magnitude;
		}
	}
	result
}

fn element_bytes(data: &[u8], offset: usize, size: usize) -> Result<&[u8]> {
	if size == 0 {
		return Ok(&[]);
	}
	checked_slice(data, offset, size)
}

fn checked_slice(data: &[u8], offset: usize, size: usize) -> Result<&[u8]> {
	let end = offset
		.checked_add(size)
		.ok_or_else(|| resource_error("resource slice offset overflow"))?;
	data.get(offset..end)
		.ok_or_else(|| resource_error("resource slice is outside the file"))
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
	let bytes: [u8; 2] = checked_slice(data, offset, 2)?
		.try_into()
		.map_err(|_| resource_error("invalid u16"))?;
	Ok(u16::from_be_bytes(bytes))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
	let bytes: [u8; 4] = checked_slice(data, offset, 4)?
		.try_into()
		.map_err(|_| resource_error("invalid u32"))?;
	Ok(u32::from_be_bytes(bytes))
}

fn read_f32(data: &[u8], offset: usize) -> Result<f32> {
	Ok(f32::from_bits(read_u32(data, offset)?))
}

fn read_vec3(data: &[u8], offset: usize) -> Result<[f32; 3]> {
	Ok([
		read_f32(data, offset)?,
		read_f32(data, offset + 4)?,
		read_f32(data, offset + 8)?,
	])
}

#[cfg(test)]
mod tests {
	use std::io::Write as _;

	use flate2::{Compression, write::ZlibEncoder};

	use super::*;

	fn test_triangle() -> Shape {
		Shape {
			positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
			texcoords: Vec::new(),
			normals: vec![[1.0, 0.0, 1.0]; 3],
			tangents: vec![[1.0, 0.0, 1.0, 1.0]; 3],
			colors: Vec::new(),
			indices: vec![0, 1, 2],
			bounds: [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
			transform: ShapeTransform::None,
		}
	}

	fn triangle_normal_z(shape: &Shape) -> f32 {
		let [a, b, c] = shape.indices[..3] else {
			panic!("test shape should contain one triangle");
		};
		let a = shape.positions[usize::from(a)];
		let b = shape.positions[usize::from(b)];
		let c = shape.positions[usize::from(c)];
		(b[1] - a[1]).mul_add(-(c[0] - a[0]), (b[0] - a[0]) * (c[1] - a[1]))
	}

	fn assert_floats_equal<const N: usize>(actual: [f32; N], expected: [f32; N]) {
		for (actual, expected) in actual.into_iter().zip(expected) {
			assert!((actual - expected).abs() < f32::EPSILON);
		}
	}

	fn afl_2_3_high_header() -> Vec<u8> {
		let mut bytes = vec![0; AFL_2_3_HIGH_HEADER_SIZE];
		bytes[..4].copy_from_slice(MAGIC);
		bytes[4..8].copy_from_slice(&VERSION.to_be_bytes());
		bytes[12..16].copy_from_slice(&AFL_2_3_HIGH_EXPANDED_SIZE.to_be_bytes());
		bytes
	}

	fn test_archive(bytes: Vec<u8>) -> ResourceArchive {
		ResourceArchive {
			bytes: bytes.into(),
			expanded_entries: Arc::new(Mutex::new(HashMap::new())),
			source_path: None,
		}
	}

	fn compress_zlib(bytes: &[u8]) -> Vec<u8> {
		let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
		encoder.write_all(bytes).expect("test data should compress");
		encoder.finish().expect("test stream should finish")
	}

	#[test]
	fn horizontal_reflection_preserves_winding_and_tangent_handedness() {
		let original = test_triangle();
		assert!(triangle_normal_z(&original) > 0.0);

		let mut reflected = original.clone();
		reflected.adjust(1.0, 1.0, [0.0; 3], true);
		assert_eq!(reflected.indices, [0, 2, 1]);
		assert!(triangle_normal_z(&reflected) > 0.0);
		assert_floats_equal(reflected.normals[0], [-1.0, 0.0, 1.0]);
		assert_floats_equal(reflected.tangents[0], [-1.0, 0.0, 1.0, -1.0]);

		let mut transformed = original;
		transformed.adjust(1.0, 1.0, [0.0; 3], false);
		assert_eq!(transformed.indices, [0, 1, 2]);
		assert!((transformed.tangents[0][3] - 1.0).abs() < f32::EPSILON);
	}

	#[test]
	fn normal_unpack_matches_signed_ten_bit_layout() {
		for component in unpack_normal(0) {
			assert!(component.abs() < f32::EPSILON);
		}
		assert!((unpack_normal(0x0000_01ff)[0] - 1.0).abs() < f32::EPSILON);
		assert!((unpack_normal(0x0000_0201)[0] - (-511.0 / 511.0)).abs() < f32::EPSILON);
	}

	#[test]
	fn afl_2_3_high_header_offsets_cover_exact_header() {
		let texture_end = BASE_HEADER_SIZE
			+ AFL_2_3_TEXTURE_COUNTS.len() * 4
			+ AFL_2_3_TEXTURE_COUNTS.iter().sum::<usize>() * PART_INFO_SIZE;
		let shape_end = texture_end
			+ SHAPE_COUNTS.len() * 4
			+ SHAPE_COUNTS.iter().sum::<usize>() * PART_INFO_SIZE;
		assert_eq!(texture_end, 0x1710);
		assert_eq!(
			texture_end - BASE_HEADER_SIZE,
			AFL_2_3_HIGH_TEXTURE_HEADER_SIZE
		);
		assert_eq!(shape_end + 48, AFL_2_3_HIGH_HEADER_SIZE);
	}

	#[test]
	fn rejects_legacy_resource_archive_layouts() {
		for expanded_size in [0x00cb_bde0_u32, 0x0239_d5e0, (2 << 29) | 0x0239_d5e0] {
			let mut bytes = vec![0; BASE_HEADER_SIZE];
			bytes[..4].copy_from_slice(MAGIC);
			bytes[4..8].copy_from_slice(&VERSION.to_be_bytes());
			bytes[12..16].copy_from_slice(&expanded_size.to_be_bytes());
			let error = ResourceArchive::unchecked_from_bytes(bytes)
				.expect_err("legacy resource archives should be rejected");
			assert!(matches!(
				error,
				Error::InvalidResource(message)
					if message == "only AFL 2.3 high resource archives are supported"
			));
		}
	}

	#[test]
	fn unchecked_archive_bypasses_trusted_digest() {
		let archive = ResourceArchive::unchecked_from_bytes(afl_2_3_high_header())
			.expect("unchecked loading should accept an AFL 2.3 high header");
		assert_eq!(archive.len(), 0x4d00);
	}

	#[test]
	fn trusted_archive_rejects_unknown_digest() {
		let error = ResourceArchive::from_bytes(afl_2_3_high_header())
			.expect_err("an unknown AFL 2.3 high archive should not be trusted");
		assert!(matches!(
			error,
			Error::UntrustedResourceArchive { sha256 } if sha256.len() == 64
		));
	}

	#[test]
	fn rejects_entries_above_expanded_size_limit() {
		let error = test_archive(Vec::new())
			.load_entry(PartInfo {
				data_pos: 0,
				data_size: MAX_EXPANDED_ENTRY_SIZE + 1,
				compressed_size: 0,
				strategy: 5,
			})
			.expect_err("oversized entries should be rejected before allocation");
		assert!(matches!(
			error,
			Error::InvalidResource(message)
				if message == format!(
					"expanded entry has {} bytes, maximum is {MAX_EXPANDED_ENTRY_SIZE}",
					MAX_EXPANDED_ENTRY_SIZE + 1
				)
		));
	}

	#[test]
	fn caps_zlib_output_at_expected_size_plus_one() {
		let compressed = compress_zlib(&[0; 4096]);
		let compressed_size = compressed.len();
		let error = test_archive(compressed)
			.load_entry(PartInfo {
				data_pos: 0,
				data_size: 8,
				compressed_size,
				strategy: 0,
			})
			.expect_err("over-expanding streams should be rejected");
		assert!(matches!(
			error,
			Error::InvalidResource(message)
				if message == "decoded entry has 9 bytes, expected 8"
		));
	}

	#[test]
	fn caches_expanded_entries_across_archive_clones() {
		let expected = [42; 4096];
		let compressed = compress_zlib(&expected);
		let compressed_size = compressed.len();
		let archive = test_archive(compressed);
		let entry = PartInfo {
			data_pos: 0,
			data_size: expected.len(),
			compressed_size,
			strategy: 0,
		};
		let archive_clone = archive.clone();

		let first = archive
			.load_entry(entry)
			.expect("first load should decompress the entry");
		drop(archive);
		let second = archive_clone
			.load_entry(entry)
			.expect("archive clones should reuse the expanded entry");

		assert_eq!(first.as_ref(), expected);
		assert!(Arc::ptr_eq(&first, &second));
		assert_eq!(
			archive_clone
				.expanded_entries
				.lock()
				.unwrap_or_else(PoisonError::into_inner)
				.len(),
			1
		);
	}

	#[test]
	fn rejects_zero_sized_textures() {
		for (width, height) in [(0_u16, 1_u16), (1, 0), (0, 0)] {
			let mut data = vec![0; 12];
			data[4..6].copy_from_slice(&width.to_be_bytes());
			data[6..8].copy_from_slice(&height.to_be_bytes());
			let error = Texture::decode(&data)
				.expect_err("zero-sized textures should be rejected during decoding");
			assert!(matches!(
				error,
				Error::InvalidResource(message)
					if message == format!(
						"texture dimensions must be nonzero, got {width}x{height}"
					)
			));
		}
	}

	#[test]
	fn afl_2_3_high_resources_include_extended_glasses() {
		let archive = test_archive(Vec::new());
		ResourceArchive::require_texture(TexturePart::Glass, 19)
			.expect("AFL 2.3 high resources contain all 20 glasses textures");
		let error = archive
			.load_texture(TexturePart::Glass, 20)
			.expect_err("glasses texture index 20 should be out of range");
		assert!(matches!(
			error,
			Error::UnsupportedResource {
				kind: ResourceKind::Texture,
				part: "glass",
				index: 20,
				available: 20,
			}
		));
	}
}
