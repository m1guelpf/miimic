/// Errors produced by Miimic's format and rendering layers.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Mii data is neither valid hexadecimal nor supported base64")]
	InvalidEncoding,

	#[error("unsupported Mii data length: {0} bytes")]
	UnsupportedDataLength(usize),

	#[error("invalid Mii data: {field}={value}; expected {min}..={max}")]
	InvalidMii {
		field: &'static str,
		value: u8,
		min: u8,
		max: u8,
	},

	#[error("invalid {0:?} Mii data: CRC-16 checksum mismatch")]
	InvalidMiiChecksum(crate::MiiFormat),

	#[error(
		"selected resource archive has no {kind} {part}[{index}] (available entries: {available})"
	)]
	UnsupportedResource {
		kind: crate::ResourceKind,
		part: &'static str,
		index: usize,
		available: usize,
	},

	#[error("invalid FFL resource: {0}")]
	InvalidResource(String),
	#[error("invalid FFL resource: {context}: {source}")]
	InvalidResourceSource {
		context: String,
		#[source]
		source: Box<dyn std::error::Error + Send + Sync>,
	},

	#[error("AFL 2.3 high resource archive is not trusted (SHA-256: {sha256})")]
	UntrustedResourceArchive { sha256: String },

	#[error("resource I/O failed: {0}")]
	Io(#[from] std::io::Error),

	#[error("invalid render request: {0}")]
	InvalidRequest(String),

	#[error("feature is not implemented yet: {0}")]
	FeatureUnavailable(&'static str),

	#[error("rendering failed: {0}")]
	Render(String),
	#[error("rendering failed: {context}: {source}")]
	RenderSource {
		context: String,
		#[source]
		source: Box<dyn std::error::Error + Send + Sync>,
	},
}

/// Result type used throughout Miimic.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
	use std::error::Error as _;

	use crate::{render_source, resource_source};

	#[test]
	fn contextual_errors_preserve_their_source() {
		let error = render_source("test operation failed", std::io::Error::other("root cause"));
		assert_eq!(
			error.to_string(),
			"rendering failed: test operation failed: root cause"
		);
		let source = error
			.source()
			.expect("contextual error should have a source");
		assert_eq!(source.to_string(), "root cause");
		assert!(source.downcast_ref::<std::io::Error>().is_some());

		let resource = resource_source(
			"test resource failed",
			std::io::Error::other("resource cause"),
		);
		assert_eq!(
			resource
				.source()
				.expect("resource error should have a source")
				.to_string(),
			"resource cause"
		);
	}
}
