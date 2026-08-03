use std::{f32::consts::TAU, ops::Range};

use num_traits::ToPrimitive as _;

use crate::{
	Expression, Mii, ModelTexture, ResourceArchive, Result, Texture, TexturePart, color,
	composite::{float_to_unorm, sample_linear},
	expression,
};

const TEX_SCALE_X: f32 = 0.889_614_64;
const TEX_SCALE_Y: f32 = 0.927_667_5;
const SPACING_MUL: f32 = 0.889_614_64;
const POS_X_MUL: f32 = 1.779_229_3;
const POS_Y_MUL: f32 = 1.076_094_3;
const POS_X_ADD: f32 = 3.532_331_2;
const POS_Y_ADD: f32 = 4.629_278;
const POS_Y_ADD_MOLE: f32 = POS_Y_ADD + 11.178_394 + 2.0 * POS_Y_MUL;

#[derive(Clone, Copy)]
enum Origin {
	Center,
	Right,
	Left,
}

#[derive(Clone, Copy)]
enum Modulation {
	Alpha([f32; 4]),
	Layered([[f32; 4]; 3]),
	Direct,
}

fn features(
	archive: &ResourceArchive,
	mii: &Mii,
	expression: Expression,
) -> Result<Vec<OwnedFeature>> {
	let resolved = expression::resolve(*mii, expression);
	let mii = &resolved.mii;
	let eye_right = archive.load_texture(TexturePart::Eye, usize::from(resolved.eye_right))?;
	let eye_left = archive.load_texture(TexturePart::Eye, usize::from(resolved.eye_left))?;
	let eyebrow = archive.load_texture(TexturePart::Eyebrow, usize::from(resolved.eyebrow))?;
	let mouth = archive.load_texture(TexturePart::Mouth, usize::from(resolved.mouth))?;

	let eye_spacing = f32::from(mii.eye_x) * SPACING_MUL;
	let eye_base_scale = f32::from(mii.eye_scale).mul_add(0.4, 1.0);
	let eye_base_scale_y = f32::from(mii.eye_aspect).mul_add(0.12, 0.64);
	let eye_scale = [
		5.343_75 * eye_base_scale,
		4.5 * eye_base_scale * eye_base_scale_y,
	];
	let eye_y = f32::from(mii.eye_y).mul_add(POS_Y_MUL, POS_Y_ADD + 13.822_246);
	let eye_rotation =
		f32::from((mii.eye_rotate + expression::eye_rotate_offset(mii.eye_type)) % 32) * TAU / 32.0;
	let eye_colors = [
		[0.0, 1.0, 1.0, 1.0],
		[1.0; 4],
		color::eye(mii.eye_color, mii.color_mode),
	];

	let eyebrow_spacing = f32::from(mii.eyebrow_x) * SPACING_MUL;
	let eyebrow_base_scale = f32::from(mii.eyebrow_scale).mul_add(0.4, 1.0);
	let eyebrow_base_scale_y = f32::from(mii.eyebrow_aspect).mul_add(0.12, 0.64);
	let eyebrow_scale = [
		5.062_5 * eyebrow_base_scale,
		4.5 * eyebrow_base_scale * eyebrow_base_scale_y,
	];
	let eyebrow_y = f32::from(mii.eyebrow_y).mul_add(POS_Y_MUL, POS_Y_ADD + 11.920_528);
	let eyebrow_rotation =
		f32::from((mii.eyebrow_rotate + eyebrow_rotate_offset(mii.eyebrow_type)) % 32) * TAU / 32.0;

	let mouth_base_scale = f32::from(mii.mouth_scale).mul_add(0.4, 1.0);
	let mouth_base_scale_y = f32::from(mii.mouth_aspect).mul_add(0.12, 0.64);
	let mouth_scale = [
		6.187_5 * mouth_base_scale,
		4.5 * mouth_base_scale * mouth_base_scale_y,
	];
	let mouth_y = f32::from(mii.mouth_y).mul_add(POS_Y_MUL, POS_Y_ADD + 24.629_572);
	let mouth_colors = [
		color::mouth(mii.mouth_color, mii.color_mode),
		color::upper_lip(mii.mouth_color, mii.color_mode),
		[1.0; 4],
	];

	let mut features = Vec::with_capacity(8);
	push_mustache(archive, mii, &mut features)?;

	let mouth_modulation = if resolved.mouth > 36 {
		Modulation::Direct
	} else {
		Modulation::Layered(mouth_colors)
	};
	features.push(OwnedFeature::new(
		mouth,
		[32.0, mouth_y],
		mouth_scale,
		0.0,
		Origin::Center,
		mouth_modulation,
	));
	features.push(OwnedFeature::new(
		eyebrow.clone(),
		[32.0 - eyebrow_spacing, eyebrow_y],
		eyebrow_scale,
		eyebrow_rotation,
		Origin::Left,
		Modulation::Alpha(color::hair(mii.eyebrow_color, mii.color_mode)),
	));
	features.push(OwnedFeature::new(
		eyebrow,
		[32.0 + eyebrow_spacing, eyebrow_y],
		eyebrow_scale,
		-eyebrow_rotation,
		Origin::Right,
		Modulation::Alpha(color::hair(mii.eyebrow_color, mii.color_mode)),
	));
	features.push(OwnedFeature::new(
		eye_right,
		[32.0 - eye_spacing, eye_y],
		eye_scale,
		eye_rotation,
		Origin::Left,
		eye_modulation(resolved.eye_right, eye_colors),
	));
	features.push(OwnedFeature::new(
		eye_left,
		[32.0 + eye_spacing, eye_y],
		eye_scale,
		-eye_rotation,
		Origin::Right,
		eye_modulation(resolved.eye_left, eye_colors),
	));
	push_mole(archive, mii, &mut features)?;

	Ok(features)
}

fn push_mustache(
	archive: &ResourceArchive,
	mii: &Mii,
	features: &mut Vec<OwnedFeature>,
) -> Result<()> {
	if mii.mustache_type == 0 {
		return Ok(());
	}
	let mustache = archive.load_texture(TexturePart::Mustache, usize::from(mii.mustache_type))?;
	let base_scale = f32::from(mii.mustache_scale).mul_add(0.4, 1.0);
	let scale = [4.5 * base_scale, 9.0 * base_scale];
	let y = f32::from(mii.mustache_y).mul_add(POS_Y_MUL, POS_Y_ADD + 27.134_275);
	let modulation = Modulation::Alpha(color::hair(mii.beard_color, mii.color_mode));
	for origin in [Origin::Left, Origin::Right] {
		features.push(OwnedFeature::new(
			mustache.clone(),
			[32.0, y],
			scale,
			0.0,
			origin,
			modulation,
		));
	}
	Ok(())
}

fn push_mole(archive: &ResourceArchive, mii: &Mii, features: &mut Vec<OwnedFeature>) -> Result<()> {
	if mii.mole_type == 0 {
		return Ok(());
	}
	let mole = archive.load_texture(TexturePart::Mole, usize::from(mii.mole_type))?;
	let mole_scale = f32::from(mii.mole_scale).mul_add(0.4, 1.0);
	let x = f32::from(mii.mole_x).mul_add(POS_X_MUL, POS_X_ADD + 14.233_834);
	let y = f32::from(mii.mole_y).mul_add(POS_Y_MUL, POS_Y_ADD_MOLE);
	features.push(OwnedFeature::new(
		mole,
		[x, y],
		[mole_scale; 2],
		0.0,
		Origin::Center,
		Modulation::Alpha([0.071, 0.059, 0.059, 1.0]),
	));
	Ok(())
}

const fn eye_modulation(index: u8, colors: [[f32; 4]; 3]) -> Modulation {
	if matches!(index, 60 | 62 | 65 | 69..=75 | 78 | 79) {
		Modulation::Direct
	} else {
		Modulation::Layered(colors)
	}
}

pub fn compose(
	archive: &ResourceArchive,
	mii: &Mii,
	expression: Expression,
	resolution: u16,
) -> Result<ModelTexture> {
	let features = features(archive, mii, expression)?;
	let pixels = compose_features(&features, resolution);
	Ok(ModelTexture {
		width: resolution,
		height: resolution,
		pixels,
	})
}

struct OwnedFeature {
	texture: Texture,
	position: [f32; 2],
	scale: [f32; 2],
	rotation: f32,
	origin: Origin,
	modulation: Modulation,
}

impl OwnedFeature {
	const fn new(
		texture: Texture,
		position: [f32; 2],
		scale: [f32; 2],
		rotation: f32,
		origin: Origin,
		modulation: Modulation,
	) -> Self {
		Self {
			texture,
			position,
			scale,
			rotation,
			origin,
			modulation,
		}
	}
}

#[derive(Clone, Copy, Default)]
struct CompositePixel {
	color: [f32; 3],
	coverage: f32,
	alpha: f32,
}

impl CompositePixel {
	const fn into_rgba(self) -> [f32; 4] {
		[self.color[0], self.color[1], self.color[2], self.alpha]
	}
}

struct PreparedFeature<'a> {
	feature: &'a OwnedFeature,
	sin: f32,
	cos: f32,
	origin_x: f32,
	x_range: Range<u16>,
	y_range: Range<u16>,
}

impl<'a> PreparedFeature<'a> {
	fn new(feature: &'a OwnedFeature, resolution: u16) -> Self {
		let sin = feature.rotation.sin();
		let cos = feature.rotation.cos();
		let origin_x = match feature.origin {
			Origin::Center => -0.5,
			Origin::Right => 0.0,
			Origin::Left => -1.0,
		};
		let mut min = [f32::INFINITY; 2];
		let mut max = [f32::NEG_INFINITY; 2];
		for local_y in [-0.5, 0.5] {
			for local_x in [origin_x, origin_x + 1.0] {
				let scaled_x = feature.scale[0] * local_x;
				let scaled_y = feature.scale[1] * local_y;
				let adjusted_x = scaled_y.mul_add(-sin, scaled_x * cos);
				let adjusted_y = scaled_y.mul_add(cos, scaled_x * sin);
				let mask_x = adjusted_x.mul_add(TEX_SCALE_X, feature.position[0]);
				let mask_y = adjusted_y.mul_add(TEX_SCALE_Y, feature.position[1]);
				min[0] = min[0].min(mask_x);
				min[1] = min[1].min(mask_y);
				max[0] = max[0].max(mask_x);
				max[1] = max[1].max(mask_y);
			}
		}

		Self {
			feature,
			sin,
			cos,
			origin_x,
			x_range: clipped_pixel_range(min[0], max[0], resolution),
			y_range: clipped_pixel_range(min[1], max[1], resolution),
		}
	}
}

fn clipped_pixel_range(min: f32, max: f32, resolution: u16) -> Range<u16> {
	let pixels_per_mask_unit = f32::from(resolution) / 64.0;
	let resolution = i32::from(resolution);
	let start = (min * pixels_per_mask_unit)
		.floor()
		.to_i32()
		.expect("feature bound fits i32")
		.saturating_sub(1)
		.clamp(0, resolution);
	let end = (max * pixels_per_mask_unit)
		.ceil()
		.to_i32()
		.expect("feature bound fits i32")
		.saturating_add(1)
		.clamp(0, resolution);
	u16::try_from(start).expect("clipped feature start fits u16")
		..u16::try_from(end).expect("clipped feature end fits u16")
}

fn compose_features(features: &[OwnedFeature], resolution: u16) -> Vec<u8> {
	let side = usize::from(resolution);
	let mut pixels = vec![CompositePixel::default(); side * side];
	for feature in features {
		let feature = PreparedFeature::new(feature, resolution);
		draw_feature(&feature, resolution, &mut pixels);
	}
	pixels
		.into_iter()
		.flat_map(|pixel| pixel.into_rgba().map(float_to_unorm))
		.collect()
}

fn draw_feature(feature: &PreparedFeature<'_>, resolution: u16, output: &mut [CompositePixel]) {
	let side = usize::from(resolution);
	let to_mask = 64.0 / f32::from(resolution);
	for y in feature.y_range.clone() {
		let mask_y = (f32::from(y) + 0.5) * to_mask;
		for x in feature.x_range.clone() {
			let mask_x = (f32::from(x) + 0.5) * to_mask;
			let translated_x = mask_x - feature.feature.position[0];
			let translated_y = mask_y - feature.feature.position[1];
			let adjusted_x = translated_x / TEX_SCALE_X;
			let adjusted_y = translated_y / TEX_SCALE_Y;
			let local_x =
				(adjusted_x * feature.cos + adjusted_y * feature.sin) / feature.feature.scale[0];
			let rotated_y = -adjusted_x * feature.sin;
			let local_y = (rotated_y + adjusted_y * feature.cos) / feature.feature.scale[1];
			if !(feature.origin_x..=feature.origin_x + 1.0).contains(&local_x)
				|| !(-0.5..=0.5).contains(&local_y)
			{
				continue;
			}
			let u_coord = match feature.feature.origin {
				Origin::Right => 1.0 - local_x,
				Origin::Center | Origin::Left => local_x - feature.origin_x,
			};
			let v_coord = local_y + 0.5;
			let sample = sample_linear(&feature.feature.texture, u_coord, v_coord);
			let source = modulate(sample, feature.feature.modulation);
			if source[3] == 0.0 {
				continue;
			}
			let destination = &mut output[usize::from(y) * side + usize::from(x)];
			let destination_coverage = destination.coverage;
			for (destination_channel, source_channel) in destination.color.iter_mut().zip(source) {
				*destination_channel = source_channel.mul_add(
					1.0 - destination_coverage,
					*destination_channel * destination_coverage,
				);
			}
			destination.coverage = source[3].max(destination_coverage);
			destination.alpha = source[3].mul_add(source[3], destination.alpha).min(1.0);
		}
	}
}

fn modulate(texture: [f32; 4], modulation: Modulation) -> [f32; 4] {
	match modulation {
		Modulation::Alpha(color) => [color[0], color[1], color[2], texture[0]],
		Modulation::Layered(colors) => [
			colors[0][0].mul_add(
				texture[0],
				colors[1][0].mul_add(texture[1], colors[2][0] * texture[2]),
			),
			colors[0][1].mul_add(
				texture[0],
				colors[1][1].mul_add(texture[1], colors[2][1] * texture[2]),
			),
			colors[0][2].mul_add(
				texture[0],
				colors[1][2].mul_add(texture[1], colors[2][2] * texture[2]),
			),
			texture[3],
		],
		Modulation::Direct => texture,
	}
}

fn eyebrow_rotate_offset(kind: u8) -> u8 {
	const ROTATE: [u8; 28] = [
		6, 6, 5, 7, 6, 7, 6, 7, 4, 7, 6, 8, 5, 5, 6, 6, 7, 7, 6, 6, 5, 6, 7, 5, 6, 6, 6, 6,
	];
	32 - ROTATE[usize::from(kind)]
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::TextureFormat;

	fn test_features() -> Vec<OwnedFeature> {
		let texture = Texture {
			width: 4,
			height: 4,
			format: TextureFormat::Rgba8,
			pixels: (0_u8..64).map(|value| value.wrapping_mul(53)).collect(),
		};
		vec![
			OwnedFeature::new(
				texture.clone(),
				[32.0, 32.0],
				[14.0, 8.0],
				0.37,
				Origin::Center,
				Modulation::Direct,
			),
			OwnedFeature::new(
				texture.clone(),
				[7.0, 10.0],
				[12.0, 10.0],
				-0.75,
				Origin::Left,
				Modulation::Alpha([0.2, 0.4, 0.6, 1.0]),
			),
			OwnedFeature::new(
				texture,
				[58.0, 56.0],
				[13.0, 9.0],
				1.1,
				Origin::Right,
				Modulation::Layered([[0.0, 1.0, 1.0, 1.0], [1.0; 4], [0.25, 0.5, 0.75, 1.0]]),
			),
		]
	}

	fn reference_compose(features: &[OwnedFeature], resolution: u16) -> Vec<u8> {
		let side = usize::from(resolution);
		let mut pixels = vec![[0.0_f32; 4]; side * side];
		for feature in features {
			reference_draw_feature(feature, resolution, &mut pixels, false);
		}
		for pixel in &mut pixels {
			pixel[3] = 0.0;
		}
		for feature in features {
			reference_draw_feature(feature, resolution, &mut pixels, true);
		}
		pixels
			.into_iter()
			.flat_map(|pixel| pixel.map(float_to_unorm))
			.collect()
	}

	fn reference_draw_feature(
		feature: &OwnedFeature,
		resolution: u16,
		output: &mut [[f32; 4]],
		alpha_only: bool,
	) {
		let side = usize::from(resolution);
		let to_mask = 64.0 / f32::from(resolution);
		let sin = feature.rotation.sin();
		let cos = feature.rotation.cos();
		let origin_x = match feature.origin {
			Origin::Center => -0.5,
			Origin::Right => 0.0,
			Origin::Left => -1.0,
		};

		for y in 0..resolution {
			let mask_y = (f32::from(y) + 0.5) * to_mask;
			for x in 0..resolution {
				let mask_x = (f32::from(x) + 0.5) * to_mask;
				let translated_x = mask_x - feature.position[0];
				let translated_y = mask_y - feature.position[1];
				let adjusted_x = translated_x / TEX_SCALE_X;
				let adjusted_y = translated_y / TEX_SCALE_Y;
				let local_x = (adjusted_x * cos + adjusted_y * sin) / feature.scale[0];
				let rotated_y = -adjusted_x * sin;
				let local_y = (rotated_y + adjusted_y * cos) / feature.scale[1];
				if !(origin_x..=origin_x + 1.0).contains(&local_x)
					|| !(-0.5..=0.5).contains(&local_y)
				{
					continue;
				}
				let u_coord = match feature.origin {
					Origin::Right => 1.0 - local_x,
					Origin::Center | Origin::Left => local_x - origin_x,
				};
				let v_coord = local_y + 0.5;
				let sample = sample_linear(&feature.texture, u_coord, v_coord);
				let source = modulate(sample, feature.modulation);
				if source[3] == 0.0 {
					continue;
				}
				let destination = &mut output[usize::from(y) * side + usize::from(x)];
				if alpha_only {
					destination[3] = source[3].mul_add(source[3], destination[3]).min(1.0);
				} else {
					let destination_alpha = destination[3];
					for (destination_channel, source_channel) in
						destination[..3].iter_mut().zip(source)
					{
						*destination_channel = source_channel.mul_add(
							1.0 - destination_alpha,
							*destination_channel * destination_alpha,
						);
					}
					destination[3] = source[3].max(destination_alpha);
				}
			}
		}
	}

	#[test]
	fn bounded_single_pass_composition_matches_full_canvas_two_pass_reference() {
		let features = test_features();
		for resolution in [2, 3, 17, 64, 127, 1024] {
			assert_eq!(
				compose_features(&features, resolution),
				reference_compose(&features, resolution),
				"composition changed at resolution {resolution}"
			);
		}
	}

	#[test]
	fn prepared_feature_bounds_exclude_uncovered_canvas() {
		let features = test_features();
		let feature = PreparedFeature::new(&features[0], 1024);
		let width = usize::from(feature.x_range.end - feature.x_range.start);
		let height = usize::from(feature.y_range.end - feature.y_range.start);
		assert!(width * height < 1024 * 1024 / 4);
	}
}
