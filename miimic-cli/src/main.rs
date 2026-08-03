#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use miimic::{Expression, MiiData, RenderRequest, Renderer};

use crate::types::{FormatArg, ViewArg};

mod types;

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
	#[command(subcommand)]
	command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
	/// Decode a Mii and report its detected representation.
	Inspect {
		#[arg(long)]
		data: String,
	},
	/// Render a Mii to PNG, TGA, or binary glTF.
	Render {
		#[arg(long)]
		data: String,
		#[arg(long, value_enum, default_value_t = FormatArg::Png)]
		format: FormatArg,
		#[arg(long, default_value_t = 512)]
		width: u16,
		#[arg(long, value_enum, default_value_t = ViewArg::Avatar)]
		view: ViewArg,
		#[arg(long, default_value_t = 0)]
		expression: u8,
		#[arg(long)]
		texture_resolution: Option<u16>,
		#[arg(long, default_value = "./FFLResHigh.dat")]
		resources: PathBuf,
		#[arg(short, long)]
		output: PathBuf,
	},
}

fn run(cli: Cli) -> anyhow::Result<()> {
	match cli.command {
		Command::Inspect { data } => {
			let mii = MiiData::decode(&data)?;
			println!("format: {:?}", mii.format());
			println!("bytes: {}", mii.len());
		},
		Command::Render {
			data,
			format,
			width,
			view,
			expression,
			texture_resolution,
			resources,
			output,
		} => {
			let mut request = RenderRequest::new(MiiData::decode(&data)?, width)?;

			request.set_view_type(view.into());
			request.set_expression(Expression::try_from(expression)?);
			if let Some(texture_resolution) = texture_resolution {
				request.set_texture_resolution(texture_resolution)?;
			}

			let bytes = Renderer::open(resources)?.render(&request, format.into())?;
			std::fs::write(output, bytes)?;
		},
	}

	Ok(())
}

fn main() -> ExitCode {
	match run(Cli::parse()) {
		Ok(()) => ExitCode::SUCCESS,
		Err(error) => {
			eprintln!("error: {error}");
			ExitCode::FAILURE
		},
	}
}
