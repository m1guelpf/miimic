use aide::axum::ApiRouter;

mod mii;
mod system;

pub fn handler() -> ApiRouter {
	ApiRouter::new()
		.merge(mii::handler())
		.merge(system::handler())
}
