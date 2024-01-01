use std::{env, sync::Arc};

use dashmap::DashMap;

use symunix::{
    config::config, errors::handle_rejection, simulate_routes, SharedSimulationState,
};
use warp::Filter;

#[tokio::main]
#[cfg_attr(
    not(debug_assertions),
    tokio::main(flavor = "multi_thread", worker_threads = 4)
)]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "ts::api=debug");
    }
    pretty_env_logger::init();

    let config_ref = &config();
    let port = config_ref.port;
    let api_key = config_ref.api_key.clone();

    let api_base = warp::path("api").and(warp::path("v1"));

    let api_base = api_key
        .as_ref()
        .map(|key| {
            log::info!(target: "ts::api", "Running with API key protection");
            let api_key_filter =
                warp::header::exact("X-API-KEY", Box::leak(key.clone().into_boxed_str()));
            api_base.and(api_key_filter).boxed()
        })
        .unwrap_or_else(|| api_base.boxed());

    let shared_state = Arc::new(SharedSimulationState {
        evms: Arc::new(DashMap::new()),
    });

    let routes = api_base
        .and(simulate_routes(config_ref.clone(), shared_state.clone()))
        .recover(handle_rejection)
        .with(warp::log("ts::api"));

    log::info!(
        target: "ts::api",
        "Starting server on port {port}"
    );
    warp::serve(routes).run(([0, 0, 0, 0], port)).await;
}
