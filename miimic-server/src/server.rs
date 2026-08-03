use aide::openapi::{self, OpenApi};
use anyhow::Result;
use axum::Extension;
use miimic::Renderer;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::Semaphore};
use tower_http::trace::TraceLayer;

use crate::{routes, shutdown::Shutdown};

const MAX_CONCURRENT_RENDERS: usize = 1;

pub async fn start(renderer: Renderer) -> Result<()> {
	let mut openapi = OpenApi {
		info: openapi::Info {
			title: "miimic-server".to_string(),
			version: option_env!("GIT_REV")
				.unwrap_or_else(|| env!("STATIC_BUILD_DATE"))
				.to_string(),
			..openapi::Info::default()
		},
		..OpenApi::default()
	};

	let shutdown = Shutdown::new()?;
	let router = routes::handler()
		.layer(Extension(Arc::new(renderer)))
		.layer(Extension(Arc::new(Semaphore::new(MAX_CONCURRENT_RENDERS))))
		.layer(TraceLayer::new_for_http())
		.finish_api(&mut openapi);

	let router = router.layer(Extension(openapi));

	let addr = SocketAddr::from((
		[0, 0, 0, 0],
		env::var("PORT").map_or(Ok(8000), |p| p.parse())?,
	));

	tracing::info!("Starting server on {addr}...");
	axum::serve(TcpListener::bind(addr).await?, router)
		.with_graceful_shutdown(shutdown.handle())
		.await?;

	Ok(())
}
