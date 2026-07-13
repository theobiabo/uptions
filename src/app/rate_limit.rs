use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode, header::RETRY_AFTER},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, Bucket>>>,
    limit: u32,
    window: Duration,
}

struct Bucket {
    count: u32,
    started_at: Instant,
}

impl RateLimiter {
    pub fn per_minute(limit: u32) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            limit,
            window: Duration::from_secs(60),
        }
    }

    fn check(&self, key: String) -> Option<Duration> {
        let now = Instant::now();
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        if buckets.len() > 10_000 {
            buckets.retain(|_, bucket| now.duration_since(bucket.started_at) < self.window);
        }

        let bucket = buckets.entry(key).or_insert(Bucket {
            count: 0,
            started_at: now,
        });

        if now.duration_since(bucket.started_at) >= self.window {
            bucket.count = 0;
            bucket.started_at = now;
        }

        if bucket.count >= self.limit {
            return Some(
                self.window
                    .saturating_sub(now.duration_since(bucket.started_at)),
            );
        }

        bucket.count += 1;
        None
    }
}

pub async fn enforce(State(limiter): State<RateLimiter>, request: Request, next: Next) -> Response {
    let key = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(address)| address.ip().to_string())
        .unwrap_or_else(|| "unknown".to_owned());

    if let Some(retry_after) = limiter.check(key) {
        let mut response = (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "success": false,
                "message": "Too many requests"
            })),
        )
            .into_response();
        let seconds = retry_after.as_secs().max(1).to_string();

        if let Ok(value) = HeaderValue::from_str(&seconds) {
            response.headers_mut().insert(RETRY_AFTER, value);
        }

        return response;
    }

    next.run(request).await
}
