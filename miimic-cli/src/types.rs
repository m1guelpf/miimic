use clap::ValueEnum;
use miimic::{OutputFormat, ViewType};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum FormatArg {
	Png,
	Tga,
	Glb,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ViewArg {
	Avatar,
	Face,
	Body,
}

impl From<ViewArg> for ViewType {
	fn from(value: ViewArg) -> Self {
		match value {
			ViewArg::Avatar => Self::Avatar,
			ViewArg::Face => Self::Face,
			ViewArg::Body => Self::Body,
		}
	}
}

impl From<FormatArg> for OutputFormat {
	fn from(value: FormatArg) -> Self {
		match value {
			FormatArg::Png => Self::Png,
			FormatArg::Tga => Self::Tga,
			FormatArg::Glb => Self::Glb,
		}
	}
}
