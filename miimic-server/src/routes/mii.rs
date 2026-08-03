use std::sync::Arc;

use aide::axum::{ApiRouter, routing::get};
use axum::{
	Extension,
	body::Body,
	extract::Query,
	http::{StatusCode, header},
	response::Response,
};
use miimic::{Error, Expression, MiiData, OutputFormat, RenderRequest, Renderer};
use tokio::sync::Semaphore;

use crate::models::{AppError, RenderQuery};

pub fn handler() -> ApiRouter {
	ApiRouter::new()
		.api_route("/miis/image.png", get(render_png))
		.api_route("/miis/image.tga", get(render_tga))
		.api_route("/miis/image.glb", get(render_glb))
}

async fn render_png(
	Query(query): Query<RenderQuery>,
	Extension(renderer): Extension<Arc<Renderer>>,
	Extension(render_slots): Extension<Arc<Semaphore>>,
) -> Result<Response, AppError> {
	render(query, OutputFormat::Png, renderer, render_slots).await
}

async fn render_tga(
	Query(query): Query<RenderQuery>,
	Extension(renderer): Extension<Arc<Renderer>>,
	Extension(render_slots): Extension<Arc<Semaphore>>,
) -> Result<Response, AppError> {
	render(query, OutputFormat::Tga, renderer, render_slots).await
}

async fn render_glb(
	Query(query): Query<RenderQuery>,
	Extension(renderer): Extension<Arc<Renderer>>,
	Extension(render_slots): Extension<Arc<Semaphore>>,
) -> Result<Response, AppError> {
	render(query, OutputFormat::Glb, renderer, render_slots).await
}

async fn render(
	mut query: RenderQuery,
	format: OutputFormat,
	renderer: Arc<Renderer>,
	render_slots: Arc<Semaphore>,
) -> Result<Response, AppError> {
	query.data.retain(|character| character != ' ');
	let mut request = RenderRequest::new(MiiData::decode(&query.data)?, query.width)?;
	request.set_expression(parse_expression(&query.expression)?);
	if let Some(texture_resolution) = query.texture_resolution {
		request.set_texture_resolution(texture_resolution)?;
	}
	request.set_view_type(query.view_type.into());

	let permit = Arc::clone(&render_slots)
		.try_acquire_owned()
		.map_err(|_| AppError::RenderCapacity)?;
	let renderer = Arc::clone(&renderer);
	let bytes = tokio::task::spawn_blocking(move || {
		let _permit = permit;
		renderer.render(&request, format)
	})
	.await??;

	let response = Response::builder()
		.status(StatusCode::OK)
		.header(header::CONTENT_TYPE, format.media_type())
		.header(
			header::CONTENT_DISPOSITION,
			format!("attachment; filename=miimic.{}", format.extension()),
		)
		.body(Body::from(bytes))?;

	Ok(response)
}

fn parse_expression(value: &str) -> miimic::Result<Expression> {
	match value {
		"normal" => Ok(Expression::Normal),
		"smile" => Ok(Expression::Smile),
		"anger" => Ok(Expression::Anger),
		"sorrow" | "puzzled" => Ok(Expression::Sorrow),
		"surprise" | "surprised" => Ok(Expression::Surprise),
		"blink" => Ok(Expression::Blink),
		"open_mouth" | "normal_open_mouth" => Ok(Expression::OpenMouth),
		"happy" | "smile_open_mouth" => Ok(Expression::SmileOpenMouth),
		"anger_open_mouth" => Ok(Expression::AngerOpenMouth),
		"sorrow_open_mouth" => Ok(Expression::SorrowOpenMouth),
		"surprise_open_mouth" => Ok(Expression::SurpriseOpenMouth),
		"blink_open_mouth" => Ok(Expression::BlinkOpenMouth),
		"wink_right" => Ok(Expression::WinkLeft),
		"wink_left" => Ok(Expression::WinkRight),
		"wink_right_open_mouth" => Ok(Expression::WinkLeftOpenMouth),
		"wink_left_open_mouth" => Ok(Expression::WinkRightOpenMouth),
		"like" | "like_wink_right" => Ok(Expression::LikeWinkLeft),
		"like_wink_left" => Ok(Expression::LikeWinkRight),
		"frustrated" => Ok(Expression::Frustrated),
		_ => {
			let expression = value
				.parse::<u8>()
				.map_err(|_| Error::InvalidRequest(format!("unknown expression: {value}")))?;
			Expression::try_from(expression).map_err(|_| {
				Error::InvalidRequest("expression must be between 0 and 69".to_owned())
			})
		},
	}
}
