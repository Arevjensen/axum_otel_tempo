use base64::{engine::general_purpose, Engine};
use opentelemetry::{
    sdk::{
        trace::{self, RandomIdGenerator, Sampler},
        Resource,
    },
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use std::{collections::HashMap, env, time::Duration};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};

struct Settings {
    otel_username: String,
    otel_password: String,
    otel_endpoint: String,
}

pub fn init() {
    let settings = load_settings();

    init_otel_telemetry(settings);
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
