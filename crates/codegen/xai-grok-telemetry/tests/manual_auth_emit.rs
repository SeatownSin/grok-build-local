//! Wire test, inverted: product-events egress has been removed from this
//! build. Even with a fully configured events endpoint and telemetry mode
//! `Enabled`, `log_event(ManualAuth)` must NOT POST anywhere. A real HTTP
//! collector is mocked so the absence is checked on the wire, not just in
//! the stubbed client.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use xai_grok_telemetry::client;
use xai_grok_telemetry::config::{TelemetryConfig, TelemetryMode};
use xai_grok_telemetry::events::{AuthTokenKind, ManualAuth, ManualAuthReason, ManualAuthSurface};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_auth_never_posts_to_events_endpoint() {
    let bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let captured = bodies.clone();
    let app = axum::Router::new().route(
        "/events",
        axum::routing::post(move |axum::Json(v): axum::Json<serde_json::Value>| {
            let captured = captured.clone();
            async move {
                captured.lock().unwrap().push(v);
                axum::http::StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/events", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    client::init(
        TelemetryConfig {
            events_url: Some(url),
            events_api_key: Some("test-key".into()),
            mixpanel_enabled: true,
            mixpanel_token: Some("test-token".into()),
            ..TelemetryConfig::default()
        },
        TelemetryMode::Enabled,
        Some("user-xyz".into()),
        None,
        None,
        None,
        "0.0.0-test".into(),
        None,
        reqwest::Client::new(),
    );

    xai_grok_telemetry::log_event(ManualAuth {
        reason: ManualAuthReason::RefreshTokenRejected,
        trigger: ManualAuthSurface::Turn,
        token_kind: AuthTokenKind::OidcSession,
        principal: Some("user-xyz".into()),
    });

    // The old emit path was fire-and-forget; give any stray task ample time
    // to hit the collector before asserting silence.
    tokio::time::sleep(Duration::from_millis(750)).await;
    assert!(
        bodies.lock().unwrap().is_empty(),
        "product-events egress must be removed: no POST may reach the collector",
    );

    server.abort();
}
