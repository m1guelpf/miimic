use std::path::Path;

use crate::{
	CharacterModel, Error, Expression, Mii, MiiData, ModelPart, ModulateMode, OutputFormat,
	RenderRequest, Renderer, ResourceArchive, ShapePart, ShapeTransform, TextureFormat,
	TexturePart, ViewType, encode_glb, render_to_glb, render_to_png,
};

const TEST_MII: &str =
	"0800600308040402020c0301060406020a0000000008000804000a2a003e7f04000214031304160d04000a040109";

fn archive() -> ResourceArchive {
	let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../FFLResHigh.dat");
	ResourceArchive::open(path).expect("control resource archive should load")
}

fn canonical_mii() -> Mii {
	Mii::from_data(&MiiData::decode(TEST_MII).expect("fixture should decode"))
		.expect("fixture should validate")
}

#[test]
fn one_shot_glb_export_uses_the_public_request_path() {
	let request = RenderRequest::new(
		MiiData::decode(TEST_MII).expect("fixture should decode"),
		64,
	)
	.expect("request should validate");
	let glb = render_to_glb(&archive(), &request).expect("one-shot GLB should render");
	assert_eq!(glb.get(..4), Some(b"glTF".as_slice()));
	let parsed = gltf::Gltf::from_slice(&glb).expect("gltf-rs should load the generated GLB");
	let blob = parsed
		.blob
		.as_ref()
		.expect("GLB should embed its binary data");
	let buffer = parsed
		.buffers()
		.next()
		.expect("GLB should declare one buffer");
	assert!(buffer.length() <= blob.len());
	assert!(blob.len() - buffer.length() < 4);
	assert!(
		parsed
			.meshes()
			.flat_map(|mesh| mesh.primitives())
			.all(|primitive| {
				primitive.attributes().any(
					|(semantic, _)| matches!(semantic, gltf::Semantic::Extras(name) if name == "COLOR"),
				) && primitive
					.extras()
					.as_ref()
					.is_some_and(|extras| extras.get().contains("\"modulateType\""))
			}),
		"every primitive should preserve Miimic's custom color attribute and metadata"
	);
	assert!(
		parsed
			.document
			.as_json()
			.asset
			.extras
			.as_ref()
			.is_some_and(|extras| extras.get().contains("\"additionalInfo\""))
	);
}

#[test]
fn one_shot_png_render_uses_the_public_request_path() {
	let mut request = RenderRequest::new(
		MiiData::decode(TEST_MII).expect("fixture should decode"),
		32,
	)
	.expect("request should validate");
	request.set_view_type(ViewType::Face);
	let png = render_to_png(&archive(), &request).expect("one-shot PNG should render");
	assert_eq!(png.get(..8), Some(b"\x89PNG\r\n\x1a\n".as_slice()));
}

#[test]
fn body_backed_views_report_missing_assets() {
	let mut request = RenderRequest::new(
		MiiData::decode(TEST_MII).expect("fixture should decode"),
		32,
	)
	.expect("request should validate");
	let renderer = Renderer::new(archive()).expect("renderer should initialize");

	for output in [OutputFormat::Png, OutputFormat::Tga] {
		let error = renderer
			.render(&request, output)
			.expect_err("avatar raster should require body assets");
		assert!(matches!(
			error,
			Error::FeatureUnavailable("Wii U body assets")
		));
	}

	request.set_view_type(ViewType::Body);
	for output in [OutputFormat::Glb, OutputFormat::Png, OutputFormat::Tga] {
		let error = renderer
			.render(&request, output)
			.expect_err("body view should require body assets");
		assert!(matches!(
			error,
			Error::FeatureUnavailable("Wii U body assets")
		));
	}
}

#[test]
fn reusable_renderer_accepts_face_in_every_output_format() {
	let mut request = RenderRequest::new(
		MiiData::decode(TEST_MII).expect("fixture should decode"),
		32,
	)
	.expect("request should validate");
	request.set_view_type(ViewType::Face);
	let renderer = Renderer::new(archive()).expect("renderer should initialize");

	for output in [OutputFormat::Glb, OutputFormat::Png, OutputFormat::Tga] {
		let bytes = renderer
			.render(&request, output)
			.expect("reusable render should succeed");
		assert!(!bytes.is_empty());
	}
}

fn mii_with_studio_field(offset: usize, value: u8) -> Mii {
	let baseline = MiiData::decode(TEST_MII).expect("fixture should decode");
	let mut bytes = baseline.as_bytes().to_vec();
	bytes[offset] = value;
	Mii::from_data(&MiiData::try_from(bytes).expect("Studio data length should be valid"))
		.expect("field variant should validate")
}

fn mii_from_studio_fields(fields: [u8; 46]) -> Mii {
	Mii::from_data(&MiiData::try_from(fields.to_vec()).expect("Studio data length should be valid"))
		.expect("boundary profile should validate")
}

fn assert_floats_equal<const N: usize>(actual: [f32; N], expected: [f32; N]) {
	for (actual, expected) in actual.into_iter().zip(expected) {
		assert!((actual - expected).abs() < f32::EPSILON);
	}
}

#[test]
fn canonical_meshes_match_control_counts_and_bounds() {
	let archive = archive();
	let mut faceline = archive
		.load_shape(ShapePart::Faceline, 0)
		.expect("faceline should decode");
	let ShapeTransform::Faceline { hair, nose, .. } = faceline.transform else {
		panic!("faceline transform should decode");
	};
	let mut nose_shape = archive
		.load_shape(ShapePart::Nose, 1)
		.expect("nose should decode");
	let mut forehead = archive
		.load_shape(ShapePart::ForeheadNormal, 62)
		.expect("forehead should decode");
	let mut hair_shape = archive
		.load_shape(ShapePart::HairNormal, 62)
		.expect("hair should decode");
	let mut mask = archive
		.load_shape(ShapePart::Mask, 0)
		.expect("mask should decode");
	let mut noseline = archive
		.load_shape(ShapePart::Noseline, 1)
		.expect("noseline should decode");

	faceline.adjust(1.0, 1.0, [0.0; 3], false);
	let nose_position = [nose[0], nose[1] - 1.5, nose[2]];
	nose_shape.adjust(1.1, 1.1, nose_position, false);
	forehead.adjust(1.0, 1.0, hair, false);
	hair_shape.adjust(1.0, 1.0, hair, false);
	mask.adjust(1.0, 1.0, [0.0; 3], false);
	noseline.adjust(1.1, 1.1, nose_position, false);

	let shapes = [faceline, nose_shape, forehead, hair_shape, mask, noseline];
	let expected = [
		(154, 702),
		(121, 501),
		(8, 15),
		(502, 2706),
		(156, 792),
		(4, 6),
	];
	for (shape, (vertices, indices)) in shapes.iter().zip(expected) {
		assert_eq!(shape.positions.len(), vertices);
		assert_eq!(shape.indices.len(), indices);
	}

	let expected_bounds = [
		[
			[-26.3, 0.033_677_556, -19.106_659],
			[26.3, 49.604_626, 24.640_848],
		],
		[
			[-3.029_298_3, 19.255_35, 8.4],
			[3.029_298_3, 32.903_427, 29.353_87],
		],
		[
			[0.0, 48.104_645, 15.003_002],
			[19.820_2, 56.636_307, 23.501_22],
		],
		[
			[-30.232_924, 6.292_574, -35.005_245],
			[30.223_007, 69.857_15, 31.473_747],
		],
		[
			[-45.923_66, -19.685_751, -18.816_889],
			[45.923_66, 80.598_49, 25.340_137],
		],
		[[-4.399_999_6, 17.5, 26.0], [1.100_000_1, 29.6, 26.0]],
	];
	for (shape, bounds) in shapes.iter().zip(expected_bounds) {
		assert_eq!(shape.bounds, bounds);
	}
}

#[test]
fn mirrored_hair_preserves_front_faces_and_encodes_glb() {
	let archive = archive();
	let normal = CharacterModel::build(&archive, canonical_mii(), Expression::Normal, 64)
		.expect("normal hair model should build");
	let mirrored = CharacterModel::build(
		&archive,
		mii_with_studio_field(28, 1),
		Expression::Normal,
		64,
	)
	.expect("mirrored hair model should build");

	for part in [ModelPart::Forehead, ModelPart::Hair] {
		let normal_mesh = normal
			.meshes
			.iter()
			.find(|mesh| mesh.part == part)
			.expect("canonical hairstyle should contain attachment");
		let mirrored_mesh = mirrored
			.meshes
			.iter()
			.find(|mesh| mesh.part == part)
			.expect("mirrored hairstyle should contain attachment");
		assert_eq!(
			normal_mesh.shape.indices.len(),
			mirrored_mesh.shape.indices.len()
		);
		let (normal_triangles, normal_remainder) = normal_mesh.shape.indices.as_chunks::<3>();
		let (mirrored_triangles, mirrored_remainder) = mirrored_mesh.shape.indices.as_chunks::<3>();
		assert!(normal_remainder.is_empty());
		assert!(mirrored_remainder.is_empty());
		for (normal_triangle, mirrored_triangle) in normal_triangles.iter().zip(mirrored_triangles)
		{
			assert_eq!(
				*mirrored_triangle,
				[normal_triangle[0], normal_triangle[2], normal_triangle[1]]
			);
		}
		for (normal_tangent, mirrored_tangent) in normal_mesh
			.shape
			.tangents
			.iter()
			.zip(&mirrored_mesh.shape.tangents)
		{
			assert_floats_equal(
				*mirrored_tangent,
				[
					-normal_tangent[0],
					normal_tangent[1],
					normal_tangent[2],
					-normal_tangent[3],
				],
			);
		}
	}

	encode_glb(&mirrored).expect("mirrored hair GLB should encode");
}

#[test]
fn bald_style_omits_empty_attachments_and_encodes_glb() {
	let archive = archive();
	let hair = archive
		.load_shape(ShapePart::HairNormal, 30)
		.expect("bald hair resource should decode");
	let forehead = archive
		.load_shape(ShapePart::ForeheadNormal, 30)
		.expect("bald forehead resource should decode");
	assert!(hair.indices.is_empty());

	let model = CharacterModel::build(
		&archive,
		mii_with_studio_field(29, 30),
		Expression::Normal,
		64,
	)
	.expect("bald model should build");
	assert!(
		model
			.meshes
			.iter()
			.all(|mesh| !mesh.shape.indices.is_empty())
	);
	assert!(model.meshes.iter().all(|mesh| mesh.part != ModelPart::Hair));
	assert_eq!(
		model
			.meshes
			.iter()
			.any(|mesh| mesh.part == ModelPart::Forehead),
		!forehead.indices.is_empty()
	);

	encode_glb(&model).expect("bald GLB should encode");
}

#[test]
fn nose_style_zero_omits_empty_nose_and_renders_png() {
	let archive = archive();
	let nose = archive
		.load_shape(ShapePart::Nose, 0)
		.expect("placeholder nose resource should decode");
	let noseline = archive
		.load_shape(ShapePart::Noseline, 0)
		.expect("placeholder noseline resource should decode");
	assert!(nose.indices.is_empty());
	assert!(!noseline.indices.is_empty());

	let model = CharacterModel::build(
		&archive,
		mii_with_studio_field(44, 0),
		Expression::Normal,
		32,
	)
	.expect("nose style zero model should build");
	assert!(
		model
			.meshes
			.iter()
			.all(|mesh| !mesh.shape.indices.is_empty())
	);
	assert!(model.meshes.iter().all(|mesh| mesh.part != ModelPart::Nose));
	assert!(
		model
			.meshes
			.iter()
			.any(|mesh| mesh.part == ModelPart::Noseline)
	);
	encode_glb(&model).expect("nose style zero GLB should encode");

	let baseline = MiiData::decode(TEST_MII).expect("fixture should decode");
	let mut bytes = baseline.as_bytes().to_vec();
	bytes[44] = 0;
	let mut request = RenderRequest::new(
		MiiData::try_from(bytes).expect("Studio data length should be valid"),
		32,
	)
	.expect("nose style zero request should validate");
	request.set_view_type(ViewType::Face);
	let png = render_to_png(&archive, &request).expect("nose style zero PNG should render");
	assert_eq!(png.get(..8), Some(b"\x89PNG\r\n\x1a\n".as_slice()));
}

#[test]
fn canonical_feature_textures_decode() {
	let archive = archive();
	for (part, index) in [
		(TexturePart::Eye, 2),
		(TexturePart::Eyebrow, 6),
		(TexturePart::Mouth, 22),
		(TexturePart::Noseline, 1),
	] {
		let texture = archive
			.load_texture(part, index)
			.expect("feature texture should decode");
		assert!(!texture.pixels.is_empty());
		assert_eq!(
			texture.pixels.len(),
			usize::from(texture.width)
				* usize::from(texture.height)
				* match texture.format {
					TextureFormat::R8 => 1,
					TextureFormat::Rg8 => 2,
					TextureFormat::Rgba8 => 4,
				},
		);
	}
}

#[test]
fn faceline_overlay_resources_decode() {
	let archive = archive();
	for (part, index) in [
		(TexturePart::FaceMakeup, 1),
		(TexturePart::Faceline, 1),
		(TexturePart::Beard, 1),
		(TexturePart::Beard, 2),
	] {
		let texture = archive
			.load_texture(part, index)
			.expect("faceline overlay texture should decode");
		assert_eq!((texture.width, texture.height), (256, 512));
		assert!(!texture.pixels.is_empty());
	}
}

#[test]
fn faceline_overlays_build_half_width_opaque_textures() {
	let archive = archive();
	let baseline = CharacterModel::build(&archive, canonical_mii(), Expression::Normal, 64)
		.expect("baseline model should build");
	let faceline = &baseline.meshes[0];
	assert_eq!(faceline.part, ModelPart::Faceline);
	assert_eq!(faceline.material.mode, ModulateMode::Constant);
	assert!(faceline.material.texture.is_none());

	let mut generated = Vec::new();
	for (offset, value) in [(18, 1), (20, 1), (1, 4), (1, 5)] {
		let mii = mii_with_studio_field(offset, value);
		let model = CharacterModel::build(&archive, mii, Expression::Normal, 64)
			.expect("faceline overlay model should build");
		let faceline = &model.meshes[0];
		assert_eq!(faceline.part, ModelPart::Faceline);
		assert_eq!(faceline.material.mode, ModulateMode::TextureDirect);
		let texture = faceline
			.material
			.texture
			.as_ref()
			.expect("faceline should have a generated texture");
		assert_eq!((texture.width, texture.height), (32, 64));
		assert!(
			texture
				.pixels
				.as_chunks::<4>()
				.0
				.iter()
				.all(|pixel| pixel[3] == u8::MAX)
		);
		generated.push(texture.pixels.clone());
	}

	for (index, pixels) in generated.iter().enumerate() {
		assert!(
			generated
				.iter()
				.enumerate()
				.all(|(other_index, other)| index == other_index || pixels != other),
			"each overlay should produce a distinct texture"
		);
	}
}

#[test]
fn beard_shape_and_texture_styles_use_their_respective_paths() {
	let archive = archive();
	let shape_mii = mii_with_studio_field(1, 3);
	let shape_model = CharacterModel::build(&archive, shape_mii, Expression::Normal, 64)
		.expect("fourth shape beard should build");
	assert!(
		shape_model
			.meshes
			.iter()
			.any(|mesh| mesh.part == ModelPart::Beard)
	);

	for beard_type in [4, 5] {
		let texture_mii = mii_with_studio_field(1, beard_type);
		let texture_model = CharacterModel::build(&archive, texture_mii, Expression::Normal, 64)
			.expect("texture beard should build");
		assert!(
			texture_model
				.meshes
				.iter()
				.all(|mesh| mesh.part != ModelPart::Beard)
		);
		assert_eq!(
			texture_model.meshes[0].material.mode,
			ModulateMode::TextureDirect
		);
	}
}

#[test]
fn textured_faceline_is_opaque_in_glb() {
	let archive = archive();
	let mii = mii_with_studio_field(18, 1);
	let model = CharacterModel::build(&archive, mii, Expression::Normal, 64)
		.expect("textured faceline model should build");
	let glb = encode_glb(&model).expect("textured faceline GLB should encode");
	let gltf = gltf::Gltf::from_slice(&glb).expect("gltf-rs should load the generated GLB");
	let faceline = gltf
		.materials()
		.find(|material| material.name() == Some("Material_OpaFaceline"))
		.expect("faceline material should exist");
	assert_eq!(faceline.alpha_mode(), gltf::material::AlphaMode::Opaque);
}

#[test]
fn studio_boundary_profiles_build_for_every_expression() {
	let archive = archive();
	let mut minimum_fields = [0; 46];
	minimum_fields[16] = 3;
	let minimums = mii_from_studio_fields(minimum_fields);
	let maximums = mii_from_studio_fields([
		99, 5, 127, 6, 99, 7, 7, 59, 12, 18, 6, 99, 11, 8, 23, 12, 18, 9, 11, 11, 11, 11, 1, 99, 7,
		19, 20, 99, 1, 131, 127, 8, 1, 16, 30, 6, 99, 8, 35, 18, 8, 5, 16, 8, 17, 18,
	]);

	for (profile, mii) in [("minimum", minimums), ("maximum", maximums)] {
		for value in 0..70 {
			let expression =
				Expression::try_from(value).expect("expression ID should be supported");
			CharacterModel::build(&archive, mii, expression, 32).unwrap_or_else(|error| {
				panic!("{profile} Studio profile failed expression {value}: {error}")
			});
		}
	}
}

#[test]
fn direct_model_build_rejects_unbounded_texture_resolution() {
	let error = CharacterModel::build(&archive(), canonical_mii(), Expression::Normal, u16::MAX)
		.expect_err("model construction must enforce the HD texture bound");
	assert!(matches!(error, Error::InvalidRequest(_)));
}
