use axum::{
	http::{StatusCode, header},
	response::{IntoResponse, Response},
};
use miimic::{Error, ViewType};

#[derive(Debug, schemars::JsonSchema, serde::Deserialize)]
pub struct RenderQuery {
	pub data: String,
	#[serde(default = "default_width")]
	pub width: u16,
	#[serde(default = "default_expression")]
	pub expression: String,
	#[serde(rename = "texResolution")]
	pub texture_resolution: Option<u16>,
	#[serde(default, rename = "type")]
	pub view_type: FflViewType,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, schemars::JsonSchema, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FflViewType {
	#[default]
	Face,
	FaceOnly,
	AllBody,
}

impl From<FflViewType> for ViewType {
	fn from(value: FflViewType) -> Self {
		match value {
			FflViewType::Face => Self::Avatar,
			FflViewType::FaceOnly => Self::Face,
			FflViewType::AllBody => Self::Body,
		}
	}
}

const fn default_width() -> u16 {
	270
}

fn default_expression() -> String {
	"normal".to_owned()
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
	#[error(transparent)]
	Miimic(#[from] miimic::Error),
	#[error("render capacity is currently exhausted")]
	RenderCapacity,
	#[error("blocking render task failed: {0}")]
	RenderTask(#[from] tokio::task::JoinError),
	#[error("failed to build HTTP response: {0}")]
	Response(#[from] axum::http::Error),
}

impl aide::OperationOutput for AppError {
	type Inner = Self;
}

impl IntoResponse for AppError {
	fn into_response(self) -> Response {
		match self {
			Self::Miimic(error) => {
				let status = match error {
					Error::InvalidEncoding
					| Error::UnsupportedDataLength(_)
					| Error::InvalidRequest(_) => StatusCode::BAD_REQUEST,
					Error::InvalidMii { .. }
					| Error::InvalidMiiChecksum(_)
					| Error::UnsupportedResource { .. } => StatusCode::UNPROCESSABLE_ENTITY,
					Error::FeatureUnavailable(_) => StatusCode::NOT_IMPLEMENTED,
					_ => StatusCode::INTERNAL_SERVER_ERROR,
				};
				(status, error.to_string()).into_response()
			},
			Self::RenderCapacity => (
				StatusCode::SERVICE_UNAVAILABLE,
				[(header::RETRY_AFTER, "1")],
				"render capacity is currently exhausted",
			)
				.into_response(),
			error @ (Self::RenderTask(_) | Self::Response(_)) => {
				(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
			},
		}
	}
}
