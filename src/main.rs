// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2024 Hyperpolymath

//! Neuro-Symbolic AI Disinformation Detector Service

mod onnx_wrapper;
mod souffle_wrapper;

use anyhow::{Context, Result};
use async_nats::jetstream::{self, consumer::PullConsumer, stream::Stream};
use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use prometheus::{Counter, Encoder, Histogram, HistogramOpts, Opts, Registry, TextEncoder};
use prost::Message;
use std::{net::SocketAddr, sync::Arc, time::Instant};
use tokio::{net::TcpListener, signal};
use tracing::{error, info, warn};

mod model_pb;

use model_pb::AnalysisInput;

const NATS_URL: &str = "nats://nats:4222";
const STREAM_NAME: &str = "INFERENCE_JOBS";
const SUBJECT_INPUT: &str = "disinfo.raw";
const CONSUMER_NAME: &str = "detector_worker";
const METRICS_PORT: u16 = 9090;

struct Metrics {
    messages_processed: Counter,
    errors: Counter,
    latency: Histogram,
    registry: Registry,
}

impl Metrics {
    fn new() -> Result<Self> {
        let registry = Registry::new();

        let messages_processed = Counter::with_opts(Opts::new(
            "nsai_messages_processed_total",
            "Total number of messages processed",
        ))?;

        let errors = Counter::with_opts(Opts::new("nsai_errors_total", "Total number of errors"))?;

        let latency = Histogram::with_opts(HistogramOpts::new(
            "nsai_processing_latency_seconds",
            "Latency of message processing",
        ))?;

        registry.register(Box::new(messages_processed.clone()))?;
        registry.register(Box::new(errors.clone()))?;
        registry.register(Box::new(latency.clone()))?;

        Ok(Self {
            messages_processed,
            errors,
            latency,
            registry,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting NSAI Detector Service (Rust Edition)");

    // Initialize ONNX runtime
    onnx_wrapper::init_runtime()?;

    // Initialize metrics
    let metrics = Arc::new(Metrics::new()?);

    // Start metrics server
    let metrics_clone = Arc::clone(&metrics);
    tokio::spawn(async move {
        if let Err(e) = run_metrics_server(metrics_clone).await {
            error!("Metrics server failed: {}", e);
        }
    });

    // Connect to NATS
    let client = async_nats::connect(NATS_URL)
        .await
        .context("Failed to connect to NATS")?;

    info!("Connected to NATS at {}", NATS_URL);

    // Get JetStream context
    let jetstream = jetstream::new(client);

    // Create or get the stream
    let stream = jetstream
        .get_or_create_stream(jetstream::stream::Config {
            name: STREAM_NAME.to_string(),
            subjects: vec![SUBJECT_INPUT.to_string()],
            ..Default::default()
        })
        .await
        .context("Failed to create stream")?;

    // Create a pull consumer
    let consumer: PullConsumer = stream
        .get_or_create_consumer(
            CONSUMER_NAME,
            jetstream::consumer::pull::Config {
                durable_name: Some(CONSUMER_NAME.to_string()),
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                deliver_policy: jetstream::consumer::DeliverPolicy::All,
                ..Default::default()
            },
        )
        .await
        .context("Failed to create consumer")?;

    info!("Listening for messages on {}...", SUBJECT_INPUT);

    // Process messages until shutdown signal
    run_consumer(consumer, stream, metrics).await
}

async fn run_consumer(
    consumer: PullConsumer,
    _stream: Stream,
    metrics: Arc<Metrics>,
) -> Result<()> {
    let mut messages = consumer
        .messages()
        .await
        .context("Failed to get message stream")?;

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Shutting down gracefully...");
                break;
            }
            msg = messages.next() => {
                match msg {
                    Some(Ok(message)) => {
                        info!("Pre-processing message: {}", message.subject);
                        process_message(&message, &metrics).await;
                        info!("Post-processing message: {}", message.subject);
                    }
                    Some(Err(e)) => {
                        warn!("Message error: {}", e);
                        metrics.errors.inc();
                    }
                    None => {
                        info!("Message stream ended");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn process_message(msg: &async_nats::jetstream::message::Message, metrics: &Metrics) {
    let start = Instant::now();

    // Parse protobuf message
    let input = match AnalysisInput::decode(msg.payload.as_ref()) {
        Ok(input) => input,
        Err(e) => {
            error!("Unmarshal error: {}", e);
            metrics.errors.inc();
            let _ = msg.ack().await;
            return;
        }
    };

    metrics.messages_processed.inc();

    // Neuro-Symbolic Pipeline
    let neural_features = match onnx_wrapper::run_inference(&input.content_hash).await {
        Ok(features) => features,
        Err(e) => {
            error!("ONNX inference error: {}", e);
            metrics.errors.inc();
            let _ = msg.ack().await;
            return;
        }
    };

    let dgraph_facts = fetch_dgraph_facts(&input.source_id).await;

    match souffle_wrapper::run_datalog(&neural_features, &dgraph_facts).await {
        Ok((verdict, explanation)) => {
            info!(
                "Verdict for {}: {} | {}",
                input.content_hash, verdict, explanation
            );
        }
        Err(e) => {
            error!("Souffle error: {}", e);
            metrics.errors.inc();
        }
    }

    metrics.latency.observe(start.elapsed().as_secs_f64());
    let _ = msg.ack().await;
}

async fn fetch_dgraph_facts(_source_id: &str) -> std::collections::HashMap<String, String> {
    // Placeholder: would query Dgraph for source reputation facts
    let mut facts = std::collections::HashMap::new();
    facts.insert("source_trusted".to_string(), "true".to_string());
    facts
}

async fn run_metrics_server(metrics: Arc<Metrics>) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], METRICS_PORT));
    let listener = TcpListener::bind(addr).await?;

    info!("Metrics server running on :{}", METRICS_PORT);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let metrics = Arc::clone(&metrics);

        tokio::spawn(async move {
            let service = service_fn(move |req: Request<hyper::body::Incoming>| {
                let metrics = Arc::clone(&metrics);
                async move { handle_metrics_request(req, metrics) }
            });

            if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                error!("Metrics connection error: {}", e);
            }
        });
    }
}

fn handle_metrics_request(
    req: Request<hyper::body::Incoming>,
    metrics: Arc<Metrics>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    if req.uri().path() == "/metrics" {
        let encoder = TextEncoder::new();
        let metric_families = metrics.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        Ok(Response::builder()
            .header("Content-Type", encoder.format_type())
            .body(Full::new(Bytes::from(buffer)))
            .unwrap())
    } else {
        Ok(Response::builder()
            .status(404)
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap())
    }
}

use futures::StreamExt;
