use axum::response::Html;
use axum::routing::get;
use axum::Router;
use base64::engine::general_purpose;
use base64::Engine;
use opentelemetry::sdk::trace::{self, RandomIdGenerator, Sampler};
use opentelemetry::sdk::Resource;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use std::collections::HashMap;
use std::env;
use std::net::TcpListener;
use std::time::Duration;
use tokio::time::sleep;
use tower_http::trace::TraceLayer;
use tracing::instrument;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

struct Settings {
    otel_username: String,
    otel_password: String,
    otel_endpoint: String,
}

#[tokio::main]
async fn main() {
    match dotenvy::dotenv() {
        Ok(path) => println!(".env read successfully from {}", path.display()),
        Err(e) => println!("Could not load .env file: {e}"),
    };

    let settings = load_settings();

    init_otel_telemetry(settings);

    // build our application with a route
    let app = Router::new()
        .route("/", get(handler))
        .layer(TraceLayer::new_for_http());

    // run it
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

fn init_otel_telemetry(settings: Settings) {
    let mut header_map = HashMap::new();
    header_map.insert(
        String::from("Authorization"),
        format!(
            "Basic {}",
            general_purpose::STANDARD
                .encode(settings.otel_username + ":" + &settings.otel_password)
        ),
    );
    let client = reqwest::Client::new();

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .http()
                .with_http_client(client)
                .with_headers(header_map)
                .with_endpoint(settings.otel_endpoint)
                .with_timeout(Duration::from_secs(3)),
        )
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_max_events_per_span(64)
                .with_max_attributes_per_span(16)
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", "axum-otel-test"),
                    KeyValue::new("environment", "dev"),
                ])),
        )
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let subscriber = Registry::default()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "axum_otel_tempo=info,tower_http=debug,axum::rejection=trace".into()
        }))
        .with(telemetry);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global default tracing");
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

fn load_settings() -> Settings {
    match dotenvy::dotenv() {
        Ok(path) => println!(".env read successfully from {}", path.display()),
        Err(e) => println!("Could not load .env file: {e}"),
    };

    Settings {
        otel_username: env::var("OtelTempoUserName").expect("OtelTempoUserName not set"),
        otel_password: env::var("OtelTempoPassword").expect("OtelTempoPassword not set"),
        otel_endpoint: env::var("OtelTempoEndpoint").expect("OtelTempoEndpoint not set"),
    }
}
