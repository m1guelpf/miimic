#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

use anyhow::Result;
use clap::Parser;
use miimic::Renderer;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

mod models;
mod routes;
mod server;
mod shutdown;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
	#[arg(long, default_value = "./FFLResHigh.dat")]
	resources: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
	dotenvy::dotenv().ok();

	tracing_subscriber::registry()
		.with(tracing_subscriber::fmt::layer().with_filter(
			EnvFilter::try_from_default_env().unwrap_or_else(|_| "miimic_server=info".into()),
		))
		.init();

	let config = Cli::parse();
	let renderer = Renderer::open(&config.resources)?;

	server::start(renderer).await
}
