#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

mod body;
mod color;
mod composite;
mod data;
mod error;
mod expression;
mod faceline;
mod glb;
mod mask;
mod mii;
mod model;
mod raster;
mod render;
mod resource;

pub use data::{MiiData, MiiFormat};
pub use error::{Error, Result};
pub use expression::Expression;
pub use render::{OutputFormat, RenderRequest, Renderer, ViewType, render_to_glb, render_to_png};
pub use resource::{ResourceArchive, ResourceKind};

use glb::{encode as encode_glb, encode_with_body as encode_glb_with_body};
use mii::{ColorMode, Mii};
use model::{CharacterModel, ModelMaterial, ModelMesh, ModelPart, ModelTexture, ModulateMode};
use resource::{Shape, ShapePart, ShapeTransform, Texture, TextureFormat, TexturePart};

#[cfg(test)]
mod control_resource_tests;

fn resource_error(message: impl Into<String>) -> Error {
	Error::InvalidResource(message.into())
}

fn resource_source(
	context: impl Into<String>,
	source: impl std::error::Error + Send + Sync + 'static,
) -> Error {
	Error::InvalidResourceSource {
		context: context.into(),
		source: Box::new(source),
	}
}

fn render_source(
	context: impl Into<String>,
	source: impl std::error::Error + Send + Sync + 'static,
) -> Error {
	Error::RenderSource {
		context: context.into(),
		source: Box::new(source),
	}
}

fn usize_from_u32(value: u32) -> Result<usize> {
	usize::try_from(value).map_err(|_| resource_error("resource size does not fit in memory"))
}
