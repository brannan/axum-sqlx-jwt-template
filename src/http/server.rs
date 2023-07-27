use crate::config::Config;
use crate::http::*;
use crate::models::{DynStore, Store};
use anyhow::Context;
use axum::Router;
use sqlx::PgPool;
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

pub async fn serve(config: Config, db: PgPool) -> anyhow::Result<()> {
    let port = config.port;

    let api_context = ApiContext {
        config: Arc::new(config),
        store: Arc::new(Store::new(db.clone())) as DynStore,
    };

    let app = api_router(api_context);

    // Port is configured in .env
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("error running HTTP server")
}

fn api_router(api_context: ApiContext) -> Router {
    // This is the order that the modules were authored in.
    Router::new()
        .merge(users::router())
        .merge(profiles::router())
        .merge(articles::router())
        // Enables logging. Use `RUST_LOG=tower_http=debug`
        .layer(TraceLayer::new_for_http())
        .with_state(api_context)
}
