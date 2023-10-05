use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use std::net::TcpListener;
use std::time::Duration;
use tokio::time::sleep;
use tower_http::trace::TraceLayer;
use tracing::instrument;
mod startup;

#[tokio::main]
async fn main() {
    startup::init();

    let app = Router::new()
        .route("/", get(handler))
        .layer(TraceLayer::new_for_http())
        .layer(OtelInResponseLayer::default())
        .layer(OtelAxumLayer::default());

    let listener = TcpListener::bind("127.0.0.1:3000").unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());

    axum::Server::from_tcp(listener.into())
        .expect("Failed to create server from listener")
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

#[instrument]
async fn handler() -> Html<&'static str> {
    Html(sub_function().await)
}

#[instrument]
async fn sub_function() -> &'static str {
    sleep(Duration::from_millis(100)).await;
    "<h1>Hi again world</h1>"
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::warn!("signal received, starting graceful shutdown");
    opentelemetry::global::shutdown_tracer_provider();
}
