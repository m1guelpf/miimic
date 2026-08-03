use std::sync::mpsc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};
use image::{ColorType, ImageEncoder as _, codecs::tga::TgaEncoder};
use wgpu::util::DeviceExt as _;

use crate::{
	CharacterModel, Error, ModelMesh, ModelPart, ModelTexture, ModulateMode, OutputFormat, Result,
	ViewType,
	body::BodyModel,
	composite::{encode_png, unorm_to_float},
	render_source,
};

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
	0 => Float32x3,
	1 => Float32x2,
	2 => Float32x3,
	3 => Float32x3,
	4 => Float32x4
];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
	position: [f32; 3],
	texcoord: [f32; 2],
	normal: [f32; 3],
	tangent: [f32; 3],
	parameter: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
	mvp: [[f32; 4]; 4],
	mv: [[f32; 4]; 4],
	normal: [[f32; 4]; 4],
	color: [f32; 4],
	material_ambient: [f32; 4],
	material_diffuse: [f32; 4],
	material_specular: [f32; 4],
	settings: [f32; 4],
}

struct GpuMesh {
	vertex: wgpu::Buffer,
	index: wgpu::Buffer,
	index_count: u32,
	bind_group: wgpu::BindGroup,
	translucent: bool,
}

/// Headless WebGPU renderer which implements the default FFL shader.
#[derive(Debug)]
pub struct Rasterizer {
	device: wgpu::Device,
	queue: wgpu::Queue,
	bind_group_layout: wgpu::BindGroupLayout,
	opaque_pipeline: wgpu::RenderPipeline,
	translucent_pipeline: wgpu::RenderPipeline,
	sampler: wgpu::Sampler,
}

impl Rasterizer {
	pub(super) fn new() -> Result<Self> {
		pollster::block_on(Self::new_async())
	}

	async fn new_async() -> Result<Self> {
		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::HighPerformance,
				force_fallback_adapter: false,
				compatible_surface: None,
				apply_limit_buckets: false,
			})
			.await
			.map_err(|error| render_source("failed to find a graphics adapter", error))?;
		let (device, queue) = adapter
			.request_device(&wgpu::DeviceDescriptor {
				label: Some("miimic renderer"),
				..Default::default()
			})
			.await
			.map_err(|error| render_source("failed to create a graphics device", error))?;
		device.on_uncaptured_error(std::sync::Arc::new(|error| {
			eprintln!("miimic graphics error: {error}");
		}));

		let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			label: Some("miimic mesh layout"),
			entries: &[
				wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 1,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Texture {
						sample_type: wgpu::TextureSampleType::Float { filterable: true },
						view_dimension: wgpu::TextureViewDimension::D2,
						multisampled: false,
					},
					count: None,
				},
				wgpu::BindGroupLayoutEntry {
					binding: 2,
					visibility: wgpu::ShaderStages::FRAGMENT,
					ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
					count: None,
				},
			],
		});
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("miimic pipeline layout"),
			bind_group_layouts: &[Some(&bind_group_layout)],
			immediate_size: 0,
		});
		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("miimic FFL shader"),
			source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
		});
		let opaque_pipeline = create_pipeline(&device, &pipeline_layout, &shader, false);
		let translucent_pipeline = create_pipeline(&device, &pipeline_layout, &shader, true);
		let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
			label: Some("miimic mirrored linear sampler"),
			address_mode_u: wgpu::AddressMode::MirrorRepeat,
			address_mode_v: wgpu::AddressMode::MirrorRepeat,
			address_mode_w: wgpu::AddressMode::MirrorRepeat,
			mag_filter: wgpu::FilterMode::Linear,
			min_filter: wgpu::FilterMode::Linear,
			mipmap_filter: wgpu::MipmapFilterMode::Linear,
			..Default::default()
		});

		Ok(Self {
			device,
			queue,
			bind_group_layout,
			opaque_pipeline,
			translucent_pipeline,
			sampler,
		})
	}

	pub(super) fn render(
		&self,
		model: &CharacterModel,
		body: Option<&BodyModel>,
		view_type: ViewType,
		width: u16,
		background: [u8; 4],
		format: OutputFormat,
	) -> Result<Vec<u8>> {
		let width = u32::from(width);
		let height = width;
		let color_texture = self.create_target(
			"miimic render color",
			COLOR_FORMAT,
			wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
			width,
			height,
		);
		let depth_texture = self.create_target(
			"miimic render depth",
			DEPTH_FORMAT,
			wgpu::TextureUsages::RENDER_ATTACHMENT,
			width,
			height,
		);
		let meshes = self.prepare_scene(model, body, view_type)?;
		let pixels = self.draw_scene(
			&color_texture,
			&depth_texture,
			&meshes,
			width,
			height,
			background,
		)?;

		match format {
			OutputFormat::Png => encode_png(width, height, &pixels),
			OutputFormat::Tga => encode_tga(width, height, &pixels),
			OutputFormat::Glb => Err(Error::InvalidRequest(
				"the raster backend cannot encode GLB".to_owned(),
			)),
		}
	}

	fn create_target(
		&self,
		label: &'static str,
		format: wgpu::TextureFormat,
		usage: wgpu::TextureUsages,
		width: u32,
		height: u32,
	) -> wgpu::Texture {
		self.device.create_texture(&wgpu::TextureDescriptor {
			label: Some(label),
			size: wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format,
			usage,
			view_formats: &[],
		})
	}

	fn prepare_scene(
		&self,
		model: &CharacterModel,
		body: Option<&BodyModel>,
		view_type: ViewType,
	) -> Result<Vec<GpuMesh>> {
		let head_translation = body.map_or(Vec3::ZERO, |body| body.head_translation);
		let head_transform = body.map_or(Mat4::IDENTITY, |body| body.head_transform);
		let (camera, target) = match view_type {
			ViewType::Body => {
				let body = body.ok_or(Error::FeatureUnavailable("Wii U body assets"))?;
				let center = body_scene_center(model, body);
				(
					Vec3::new(center.x, center.y, 900.0),
					Vec3::new(center.x, center.y, 0.0),
				)
			},
			ViewType::Avatar => (
				Vec3::new(0.0, 4.805 / 0.14, 57.553 / 0.14) + head_translation,
				Vec3::new(0.0, 4.805 / 0.14, 0.0) + head_translation,
			),
			ViewType::Face => (
				Vec3::new(0.0, 4.805 / 0.14, 57.553 / 0.14),
				Vec3::new(0.0, 4.805 / 0.14, 0.0),
			),
		};
		let view = Mat4::look_at_rh(camera, target, Vec3::Y);
		let projection = Mat4::perspective_rh(15.0_f32.to_radians(), 1.0, 10.0, 1200.0);
		let mut meshes = Vec::new();
		if let Some(body) = body {
			meshes.extend(
				body.meshes
					.iter()
					.map(|mesh| self.prepare_mesh(mesh, view, projection, Mat4::IDENTITY))
					.collect::<Result<Vec<_>>>()?,
			);
		}
		meshes.extend(
			model
				.meshes
				.iter()
				.map(|mesh| self.prepare_mesh(mesh, view, projection, head_transform))
				.collect::<Result<Vec<_>>>()?,
		);

		Ok(meshes)
	}

	fn draw_scene(
		&self,
		color_texture: &wgpu::Texture,
		depth_texture: &wgpu::Texture,
		meshes: &[GpuMesh],
		width: u32,
		height: u32,
		background: [u8; 4],
	) -> Result<Vec<u8>> {
		let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
		let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
		let padded_bytes_per_row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
		let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("miimic render readback"),
			size: u64::from(padded_bytes_per_row) * u64::from(height),
			usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
			mapped_at_creation: false,
		});
		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("miimic render encoder"),
			});
		self.record_render_pass(&mut encoder, &color_view, &depth_view, meshes, background);
		encoder.copy_texture_to_buffer(
			wgpu::TexelCopyTextureInfo {
				texture: color_texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::TexelCopyBufferInfo {
				buffer: &output_buffer,
				layout: wgpu::TexelCopyBufferLayout {
					offset: 0,
					bytes_per_row: Some(padded_bytes_per_row),
					rows_per_image: Some(height),
				},
			},
			wgpu::Extent3d {
				width,
				height,
				depth_or_array_layers: 1,
			},
		);
		self.queue.submit([encoder.finish()]);
		self.read_pixels(&output_buffer, width, height, padded_bytes_per_row)
	}

	fn record_render_pass(
		&self,
		encoder: &mut wgpu::CommandEncoder,
		color_view: &wgpu::TextureView,
		depth_view: &wgpu::TextureView,
		meshes: &[GpuMesh],
		background: [u8; 4],
	) {
		let color_attachments = [Some(wgpu::RenderPassColorAttachment {
			view: color_view,
			depth_slice: None,
			resolve_target: None,
			ops: wgpu::Operations {
				load: wgpu::LoadOp::Clear(clear_color_from_unorm8(background)),
				store: wgpu::StoreOp::Store,
			},
		})];
		let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("miimic render pass"),
			color_attachments: &color_attachments,
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: depth_view,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Clear(1.0),
					store: wgpu::StoreOp::Store,
				}),
				stencil_ops: None,
			}),
			..Default::default()
		});
		for translucent in [false, true] {
			pass.set_pipeline(if translucent {
				&self.translucent_pipeline
			} else {
				&self.opaque_pipeline
			});
			for mesh in meshes.iter().filter(|mesh| mesh.translucent == translucent) {
				pass.set_bind_group(0, &mesh.bind_group, &[]);
				pass.set_vertex_buffer(0, mesh.vertex.slice(..));
				pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint16);
				pass.draw_indexed(0..mesh.index_count, 0, 0..1);
			}
		}
	}

	fn read_pixels(
		&self,
		output_buffer: &wgpu::Buffer,
		width: u32,
		height: u32,
		padded_bytes_per_row: u32,
	) -> Result<Vec<u8>> {
		let slice = output_buffer.slice(..);
		let (sender, receiver) = mpsc::sync_channel(1);
		slice.map_async(wgpu::MapMode::Read, move |result| {
			let _ = sender.send(result);
		});
		self.device
			.poll(wgpu::PollType::wait_indefinitely())
			.map_err(|error| render_source("failed while waiting for the GPU", error))?;
		receiver
			.recv()
			.map_err(|error| render_source("GPU readback callback failed", error))?
			.map_err(|error| render_source("failed to map rendered pixels", error))?;
		let mapped = slice
			.get_mapped_range()
			.map_err(|error| render_source("failed to access rendered pixels", error))?;
		let row_bytes = usize::try_from(width * 4).expect("render width fits usize");
		let padded_row_bytes = usize::try_from(padded_bytes_per_row).expect("row size fits usize");
		let mut pixels =
			Vec::with_capacity(row_bytes * usize::try_from(height).expect("height fits"));
		for row in mapped.chunks_exact(padded_row_bytes) {
			pixels.extend_from_slice(&row[..row_bytes]);
		}
		drop(mapped);
		output_buffer.unmap();
		Ok(pixels)
	}

	fn prepare_mesh(
		&self,
		mesh: &ModelMesh,
		view: Mat4,
		projection: Mat4,
		model: Mat4,
	) -> Result<GpuMesh> {
		let vertex_count = mesh.shape.positions.len();
		let vertices = (0..vertex_count)
			.map(|index| Vertex {
				position: mesh.shape.positions[index],
				texcoord: repeated(&mesh.shape.texcoords, index, [0.0; 2]),
				normal: repeated(&mesh.shape.normals, index, [0.0, 0.0, 1.0]),
				tangent: repeated(&mesh.shape.tangents, index, [0.0; 4])[..3]
					.try_into()
					.expect("three-component slice"),
				parameter: repeated(&mesh.shape.colors, index, [u8::MAX, u8::MAX, 0, u8::MAX])
					.map(unorm_to_float),
			})
			.collect::<Vec<_>>();
		let vertex = self
			.device
			.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("miimic vertex buffer"),
				contents: bytemuck::cast_slice(&vertices),
				usage: wgpu::BufferUsages::VERTEX,
			});
		let index = self
			.device
			.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("miimic index buffer"),
				contents: bytemuck::cast_slice(&mesh.shape.indices),
				usage: wgpu::BufferUsages::INDEX,
			});
		let mv = view * model;
		let normal = Mat4::from_mat3(Mat3::from_mat4(mv).inverse().transpose());
		let material = material(mesh.part);
		let uniforms = Uniforms {
			mvp: (projection * mv).to_cols_array_2d(),
			mv: mv.to_cols_array_2d(),
			normal: normal.to_cols_array_2d(),
			color: mesh.material.color,
			material_ambient: material.ambient,
			material_diffuse: material.diffuse,
			material_specular: material.specular,
			settings: [
				material.power,
				f32::from(material.anisotropic),
				modulate_mode(mesh.material.mode),
				material.rim,
			],
		};
		let uniform = self
			.device
			.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("miimic mesh uniforms"),
				contents: bytemuck::bytes_of(&uniforms),
				usage: wgpu::BufferUsages::UNIFORM,
			});
		let texture = self.create_texture(mesh.material.texture.as_ref());
		let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("miimic mesh bind group"),
			layout: &self.bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: uniform.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::TextureView(&texture_view),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::Sampler(&self.sampler),
				},
			],
		});

		Ok(GpuMesh {
			vertex,
			index,
			index_count: u32::try_from(mesh.shape.indices.len())
				.map_err(|_| Error::Render("mesh has too many indices".to_owned()))?,
			bind_group,
			translucent: mesh.part.is_translucent(),
		})
	}

	fn create_texture(&self, texture: Option<&ModelTexture>) -> wgpu::Texture {
		let white = ModelTexture {
			width: 1,
			height: 1,
			pixels: vec![u8::MAX; 4],
		};
		let texture = texture.unwrap_or(&white);
		self.device.create_texture_with_data(
			&self.queue,
			&wgpu::TextureDescriptor {
				label: Some("miimic material texture"),
				size: wgpu::Extent3d {
					width: u32::from(texture.width),
					height: u32::from(texture.height),
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: COLOR_FORMAT,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
				view_formats: &[],
			},
			wgpu::util::TextureDataOrder::LayerMajor,
			&texture.pixels,
		)
	}
}

fn body_scene_center(model: &CharacterModel, body: &BodyModel) -> Vec3 {
	let mut min = Vec3::splat(f32::INFINITY);
	let mut max = Vec3::splat(f32::NEG_INFINITY);
	for mesh in &body.meshes {
		extend_scene_bounds(mesh, Mat4::IDENTITY, &mut min, &mut max);
	}
	for mesh in &model.meshes {
		if mesh.part != ModelPart::Mask {
			extend_scene_bounds(mesh, body.head_transform, &mut min, &mut max);
		}
	}
	(min + max) * 0.5
}

fn extend_scene_bounds(mesh: &ModelMesh, transform: Mat4, min: &mut Vec3, max: &mut Vec3) {
	for position in &mesh.shape.positions {
		let position = transform.transform_point3(Vec3::from_array(*position));
		*min = min.min(position);
		*max = max.max(position);
	}
}

fn create_pipeline(
	device: &wgpu::Device,
	layout: &wgpu::PipelineLayout,
	shader: &wgpu::ShaderModule,
	translucent: bool,
) -> wgpu::RenderPipeline {
	let blend = translucent.then_some(wgpu::BlendState {
		color: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::SrcAlpha,
			dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
			operation: wgpu::BlendOperation::Add,
		},
		alpha: wgpu::BlendComponent {
			src_factor: wgpu::BlendFactor::SrcAlpha,
			dst_factor: wgpu::BlendFactor::One,
			operation: wgpu::BlendOperation::Add,
		},
	});
	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some(if translucent {
			"miimic translucent pipeline"
		} else {
			"miimic opaque pipeline"
		}),
		layout: Some(layout),
		vertex: wgpu::VertexState {
			module: shader,
			entry_point: Some("vertex_main"),
			compilation_options: wgpu::PipelineCompilationOptions::default(),
			buffers: &[Some(wgpu::VertexBufferLayout {
				array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
				step_mode: wgpu::VertexStepMode::Vertex,
				attributes: &VERTEX_ATTRIBUTES,
			})],
		},
		primitive: wgpu::PrimitiveState {
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: Some(wgpu::Face::Back),
			..Default::default()
		},
		depth_stencil: Some(wgpu::DepthStencilState {
			format: DEPTH_FORMAT,
			depth_write_enabled: Some(!translucent),
			depth_compare: Some(if translucent {
				wgpu::CompareFunction::Less
			} else {
				wgpu::CompareFunction::LessEqual
			}),
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}),
		multisample: wgpu::MultisampleState::default(),
		fragment: Some(wgpu::FragmentState {
			module: shader,
			entry_point: Some("fragment_main"),
			compilation_options: wgpu::PipelineCompilationOptions::default(),
			targets: &[Some(wgpu::ColorTargetState {
				format: COLOR_FORMAT,
				blend,
				write_mask: wgpu::ColorWrites::ALL,
			})],
		}),
		multiview_mask: None,
		cache: None,
	})
}

#[derive(Clone, Copy)]
struct Material {
	ambient: [f32; 4],
	diffuse: [f32; 4],
	specular: [f32; 4],
	power: f32,
	anisotropic: u8,
	rim: f32,
}

fn material(part: ModelPart) -> Material {
	match part {
		ModelPart::Faceline | ModelPart::Forehead => Material {
			ambient: [0.85, 0.75, 0.75, 1.0],
			diffuse: [0.75, 0.75, 0.75, 1.0],
			specular: [0.30, 0.30, 0.30, 1.0],
			power: 1.2,
			anisotropic: 0,
			rim: 0.3,
		},
		ModelPart::Nose => Material {
			ambient: [0.90, 0.85, 0.85, 1.0],
			diffuse: [0.75, 0.75, 0.75, 1.0],
			specular: [0.22, 0.22, 0.22, 1.0],
			power: 1.5,
			anisotropic: 0,
			rim: 0.3,
		},
		ModelPart::Hair => Material {
			ambient: [1.0; 4],
			diffuse: [0.70, 0.70, 0.70, 1.0],
			specular: [0.35, 0.35, 0.35, 1.0],
			power: 10.0,
			anisotropic: 1,
			rim: 0.3,
		},
		ModelPart::Beard | ModelPart::Mask | ModelPart::Noseline | ModelPart::Glass => Material {
			ambient: [1.0; 4],
			diffuse: [0.70, 0.70, 0.70, 1.0],
			specular: [0.0, 0.0, 0.0, 1.0],
			power: 40.0,
			anisotropic: u8::from(matches!(
				part,
				ModelPart::Mask | ModelPart::Noseline | ModelPart::Glass
			)),
			rim: 0.3,
		},
		ModelPart::Body => Material {
			ambient: [0.956_22, 0.956_22, 0.956_22, 1.0],
			diffuse: [0.496_733, 0.496_733, 0.496_733, 1.0],
			specular: [0.240_9, 0.240_9, 0.240_9, 1.0],
			power: 3.0,
			anisotropic: 0,
			rim: 0.4,
		},
		ModelPart::Pants => Material {
			ambient: [0.956_22, 0.956_22, 0.956_22, 1.0],
			diffuse: [1.084_967, 1.084_967, 1.084_967, 1.0],
			specular: [0.240_9, 0.240_9, 0.240_9, 1.0],
			power: 3.0,
			anisotropic: 0,
			rim: 0.4,
		},
	}
}

const fn modulate_mode(mode: ModulateMode) -> f32 {
	match mode {
		ModulateMode::Constant => 0.0,
		ModulateMode::TextureDirect => 1.0,
		ModulateMode::Alpha => 3.0,
		ModulateMode::LuminanceAlpha => 4.0,
	}
}

fn repeated<T: Copy, const N: usize>(values: &[[T; N]], index: usize, default: [T; N]) -> [T; N] {
	values
		.get(index)
		.or_else(|| values.first())
		.copied()
		.unwrap_or(default)
}

fn clear_color_from_unorm8(background: [u8; 4]) -> wgpu::Color {
	let [r, g, b, a] = background.map(|channel| f64::from(channel) / f64::from(u8::MAX));
	wgpu::Color { r, g, b, a }
}

fn encode_tga(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>> {
	let mut bytes = Vec::new();
	TgaEncoder::new(&mut bytes)
		.disable_rle()
		.write_image(pixels, width, height, ColorType::Rgba8.into())
		.map_err(|error| render_source("failed to encode TGA", error))?;
	Ok(bytes)
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use image::{ImageDecoder as _, codecs::tga::TgaDecoder};

	use super::*;

	#[test]
	fn clear_color_maps_the_full_byte_range_to_the_unit_interval() {
		let color = clear_color_from_unorm8([u8::MAX, 128, 1, 0]);

		assert!((color.r - 1.0).abs() < f64::EPSILON);
		assert!((color.g - 128.0 / 255.0).abs() < f64::EPSILON);
		assert!((color.b - 1.0 / 255.0).abs() < f64::EPSILON);
		assert!(color.a.abs() < f64::EPSILON);
	}

	#[test]
	fn tga_encoding_preserves_rgba_pixels() {
		let pixels = [
			255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 255, 255, 255, 0,
		];
		let encoded = encode_tga(2, 2, &pixels).expect("TGA should encode");
		let decoder = TgaDecoder::new(Cursor::new(encoded)).expect("TGA should decode");
		assert_eq!(decoder.dimensions(), (2, 2));
		assert_eq!(decoder.color_type(), ColorType::Rgba8);
		let mut round_tripped = vec![0; pixels.len()];
		decoder
			.read_image(&mut round_tripped)
			.expect("TGA pixels should decode");
		assert_eq!(round_tripped, pixels);
	}
}
