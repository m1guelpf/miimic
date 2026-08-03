use aide::{axum::ApiRouter, openapi::OpenApi, scalar::Scalar};
use axum::{Extension, http::StatusCode, routing::get};
use axum_jsonschema::Json;

pub fn handler() -> ApiRouter {
	let scalar = Scalar::new("/openapi.json").with_title("miimic-server docs");

	ApiRouter::new()
		.route("/", get(get_info))
		.route("/docs", scalar.axum_route())
		.route("/openapi.json", get(api_schema))
		.route("/healtz", get(|| async { StatusCode::NO_CONTENT }))
}

#[derive(Debug, serde::Serialize)]
pub struct AppVersion {
	semver: String,
	rev: Option<String>,
	compile_time: String,
}

#[derive(Debug, serde::Serialize)]
pub struct RootResponse {
	/// Repository URL
	pub repo_url: String,

	/// Application version
	pub version: AppVersion,

	/// Documentation URL
	pub docs_url: String,
}

#[allow(clippy::unused_async)]
async fn get_info() -> Json<RootResponse> {
	Json(RootResponse {
		docs_url: "/docs".to_string(),
		repo_url: "https://github.com/m1guelpf/miimic".to_string(),
		version: AppVersion {
			semver: env!("CARGO_PKG_VERSION").to_string(),
			compile_time: env!("STATIC_BUILD_DATE").to_string(),
			rev: option_env!("GIT_REV").map(ToString::to_string),
		},
	})
}

#[allow(clippy::unused_async)]
async fn api_schema(Extension(openapi): Extension<OpenApi>) -> Json<OpenApi> {
	Json(openapi)
}
