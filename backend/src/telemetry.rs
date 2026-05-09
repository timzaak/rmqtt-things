use crate::config::OtelConfig;
use anyhow::{Context, anyhow};
use opentelemetry::global;
use opentelemetry_otlp::{MetricExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

pub fn init_telemetry(config: &OtelConfig, log_filter: fn() -> EnvFilter) -> anyhow::Result<()> {
    if let Some(ref log) = config.log {
        init_log(log_filter, log)?;
    } else {
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_env_filter(log_filter())
            .init();
    }
    if let Some(ref trace) = config.trace {
        init_traces(trace)?;
    }
    if let Some(ref metrics) = config.metrics {
        init_metrics(metrics)?;
    }
    Ok(())
}
fn get_resource() -> Resource {
    Resource::builder()
        .with_service_name("rmqtt-things")
        .build()
}

pub fn init_traces(endpoint: &str) -> anyhow::Result<()> {
    let exporter = SpanExporter::builder();
    let exporter = if endpoint.starts_with("grpc") {
        exporter
            .with_tonic()
            .with_protocol(Protocol::Grpc)
            .with_endpoint(endpoint)
            .build()
    } else {
        exporter
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(endpoint)
            .build()
    }
    .with_context(|| anyhow!("Failed to create trace exporter"))?;

    let tracer = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(get_resource())
        .build();
    global::set_tracer_provider(tracer);
    Ok(())
}

pub fn init_log(log_filter: fn() -> EnvFilter, endpoint: &str) -> anyhow::Result<()> {
    let resource = get_resource();
    let otlp_exporter = opentelemetry_otlp::LogExporter::builder();
    let otlp_exporter = if endpoint.starts_with("grpc") {
        otlp_exporter
            .with_tonic()
            .with_protocol(Protocol::Grpc)
            .with_endpoint(endpoint)
            .build()
    } else {
        otlp_exporter
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(endpoint)
            .build()
    }?;

    let log_provider = opentelemetry_sdk::logs::SdkLoggerProvider::builder()
        .with_batch_exporter(otlp_exporter)
        .with_resource(resource)
        .build();
    let otel_layer =
        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&log_provider)
            .with_filter(log_filter());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_filter(log_filter());
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(otel_layer)
        .init();
    Ok(())
}

pub fn init_metrics(endpoint: &str) -> anyhow::Result<()> {
    let exporter = MetricExporter::builder();
    let exporter = if endpoint.starts_with("grpc") {
        exporter.with_tonic().with_protocol(Protocol::Grpc).build()
    } else {
        exporter
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .build()
    }?;

    let provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_resource(get_resource())
        .build();
    global::set_meter_provider(provider);
    Ok(())
}
