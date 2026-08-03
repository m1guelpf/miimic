use std::{
	error::Error,
	fmt,
	fmt::Display,
	future::Future,
	sync::atomic::{AtomicBool, Ordering},
};
use tokio::{signal, sync::broadcast};

#[derive(Debug, PartialEq, Eq)]
pub struct AlreadyCreatedError;

impl Error for AlreadyCreatedError {}

impl Display for AlreadyCreatedError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("shutdown handler already created")
	}
}

static CREATED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct Shutdown {
	pub sender: broadcast::Sender<()>,
}

impl Shutdown {
	pub fn new() -> Result<Self, AlreadyCreatedError> {
		if (CREATED).swap(true, Ordering::SeqCst) {
			tracing::error!("shutdown handler called twice");
			return Err(AlreadyCreatedError);
		}

		let (tx, _) = broadcast::channel(1);
		let handle = register_handlers();

		let tx_for_handle = tx.clone();
		tokio::spawn(async move {
			tracing::debug!("Registered shutdown handlers");
			handle.await;
			tx_for_handle.send(()).ok();
		});

		Ok(Self { sender: tx })
	}

	pub fn handle(&self) -> impl Future<Output = ()> + Send + 'static {
		let mut rx = self.sender.subscribe();

		async move {
			let rx = rx.recv();

			let _ = rx.await;
		}
	}
}

fn register_handlers() -> impl Future<Output = ()> {
	let ctrl_c = async {
		signal::ctrl_c()
			.await
			.expect("failed to install Ctrl+C handler");
	};

	#[cfg(unix)]
	let terminate = async {
		signal::unix::signal(signal::unix::SignalKind::terminate())
			.expect("failed to install signal handler")
			.recv()
			.await;
	};

	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	async move {
		tokio::select! {
			() = ctrl_c => {
				tracing::info!("Received Ctrl+C signal");
			},
			() = terminate => {
				tracing::info!("Received terminate signal");
			},
		}
	}
}
