use std::io::Cursor;

use image::{
	ColorType, ImageEncoder as _,
	codecs::png::{CompressionType, FilterType, PngEncoder},
};
use num_traits::ToPrimitive as _;

use crate::{Result, Texture, TextureFormat, render_source};

pub fn sample_linear(texture: &Texture, u_coord: f32, v_coord: f32) -> [f32; 4] {
	let x_coord = u_coord.mul_add(f32::from(texture.width), -0.5);
	let y_coord = v_coord.mul_add(f32::from(texture.height), -0.5);
	let x_floor = x_coord.floor();
	let y_floor = y_coord.floor();
	let x_index = x_floor.to_i32().expect("texture coordinate fits i32");
	let y_index = y_floor.to_i32().expect("texture coordinate fits i32");
	let x_fraction = x_coord - x_floor;
	let y_fraction = y_coord - y_floor;
	let texel_a = sample_texel(texture, x_index, y_index);
	let texel_b = sample_texel(texture, x_index + 1, y_index);
	let texel_c = sample_texel(texture, x_index, y_index + 1);
	let texel_d = sample_texel(texture, x_index + 1, y_index + 1);
	std::array::from_fn(|channel| {
		let top = texel_a[channel].mul_add(1.0 - x_fraction, texel_b[channel] * x_fraction);
		let bottom = texel_c[channel].mul_add(1.0 - x_fraction, texel_d[channel] * x_fraction);
		top.mul_add(1.0 - y_fraction, bottom * y_fraction)
	})
}

fn sample_texel(texture: &Texture, x: i32, y: i32) -> [f32; 4] {
	let x = mirror_index(x, i32::from(texture.width));
	let y = mirror_index(y, i32::from(texture.height));
	let pixel = usize::try_from(y).expect("mirrored Y is non-negative")
		* usize::from(texture.width)
		+ usize::try_from(x).expect("mirrored X is non-negative");
	let channel = |offset: usize| unorm_to_float(texture.pixels[offset]);
	match texture.format {
		TextureFormat::R8 => {
			let red = channel(pixel);
			[red, 0.0, 0.0, 1.0]
		},
		TextureFormat::Rg8 => {
			let offset = pixel * 2;
			[channel(offset), channel(offset + 1), 0.0, 1.0]
		},
		TextureFormat::Rgba8 => {
			let offset = pixel * 4;
			[
				channel(offset),
				channel(offset + 1),
				channel(offset + 2),
				channel(offset + 3),
			]
		},
	}
}

const fn mirror_index(index: i32, size: i32) -> i32 {
	let period = size * 2;
	let wrapped = index.rem_euclid(period);
	if wrapped < size {
		wrapped
	} else {
		period - wrapped - 1
	}
}

pub fn float_to_unorm(value: f32) -> u8 {
	(value.clamp(0.0, 1.0) * 255.0)
		.round()
		.to_u8()
		.expect("clamped color channel fits u8")
}

pub fn unorm_to_float(value: u8) -> f32 {
	f32::from(value) / 255.0
}

pub fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>> {
	let mut bytes = Vec::new();
	PngEncoder::new_with_quality(
		Cursor::new(&mut bytes),
		CompressionType::Best,
		FilterType::Adaptive,
	)
	.write_image(pixels, width, height, ColorType::Rgba8.into())
	.map_err(|error| render_source("failed to encode PNG", error))?;
	Ok(bytes)
}
