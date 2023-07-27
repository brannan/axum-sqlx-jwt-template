use crate::{config::Config, models::DynStore};
use std::sync::Arc;
/// The core type through which handler functions can access common API state.
/// This can be accessed by adding a parameter `State<ApiContext>` to a handler function's
/// parameters.
///
/// In other projects I've passed this stuff as separate objects, e.g.
/// using a separate actix-web `Data` extractor for each of `Config`, `PgPool`, etc.
/// It just ends up being kind of annoying that way, but does have the whole
/// "pass only what you need where you need it" angle.
///
/// It may not be a bad idea if you need your API to be more modular (turn routes
/// on and off, and disable any unused extension objects) but it's really up to a
/// judgement call.
#[derive(Clone)]
pub struct ApiContext {
    pub config: Arc<Config>,
    pub store: DynStore,
}
