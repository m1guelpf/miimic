use crate::{
	Mii, ModelTexture, ResourceArchive, Result, Texture, TexturePart, color,
	composite::{float_to_unorm, sample_linear},
};

#[derive(Clone, Copy)]
enum Modulation {
	Direct,
	Alpha([f32; 4]),
}

pub const fn is_enabled(mii: &Mii) -> bool {
	mii.faceline_make != 0 || mii.faceline_wrinkle != 0 || mii.beard_type >= 4
}

pub fn compose(archive: &ResourceArchive, mii: &Mii, resolution: u16) -> Result<ModelTexture> {
	let width = resolution / 2;
	let skin = color::faceline(mii.faceline_color);
	let mut float_pixels = vec![skin; usize::from(width) * usize::from(resolution)];

	if mii.faceline_make != 0 {
		let makeup =
			archive.load_texture(TexturePart::FaceMakeup, usize::from(mii.faceline_make))?;
		draw_layer(
			&makeup,
			Modulation::Direct,
			width,
			resolution,
			&mut float_pixels,
		);
	}
	if mii.faceline_wrinkle != 0 {
		let wrinkle =
			archive.load_texture(TexturePart::Faceline, usize::from(mii.faceline_wrinkle))?;
		draw_layer(
			&wrinkle,
			Modulation::Alpha([0.0, 0.0, 0.0, 1.0]),
			width,
			resolution,
			&mut float_pixels,
		);
	}
	if mii.beard_type >= 4 {
		let beard = archive.load_texture(TexturePart::Beard, usize::from(mii.beard_type - 3))?;
		draw_layer(
			&beard,
			Modulation::Alpha(color::hair(mii.beard_color, mii.color_mode)),
			width,
			resolution,
			&mut float_pixels,
		);
	}

	let pixels = float_pixels
		.into_iter()
		.flat_map(|pixel| pixel.map(float_to_unorm))
		.collect();
	Ok(ModelTexture {
		width,
		height: resolution,
		pixels,
	})
}

fn draw_layer(
	texture: &Texture,
	modulation: Modulation,
	width: u16,
	height: u16,
	output: &mut [[f32; 4]],
) {
	for y in 0..height {
		let v_coord = (f32::from(y) + 0.5) / f32::from(height);
		for x in 0..width {
			let u_coord = (f32::from(x) + 0.5) / f32::from(width);
			let sampled = sample_linear(texture, u_coord, v_coord);
			let source = match modulation {
				Modulation::Direct => sampled,
				Modulation::Alpha(color) => [color[0], color[1], color[2], sampled[0]],
			};
			let destination = &mut output[usize::from(y) * usize::from(width) + usize::from(x)];
			let inverse_alpha = 1.0 - source[3];
			for channel in 0..3 {
				destination[channel] =
					source[channel].mul_add(source[3], destination[channel] * inverse_alpha);
			}
			destination[3] = (source[3] + destination[3]).min(1.0);
		}
	}
}
