use std::{convert::Infallible, time::Duration};

use axum::{
    extract::State,
    http::HeaderMap,
    response::sse::{Event, Sse},
};
use serde_json::json;
use tokio::{sync::mpsc, time};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    error::{AppError, ErrorResponse},
    notifications::dto::AutomationAlertStreamEvent,
};

#[utoipa::path(
    get,
    path = "/api/v1/automation-alerts/stream",
    tag = "Builder",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Server-sent automation alert stream", body = AutomationAlertStreamEvent, content_type = "text/event-stream"),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn stream_alerts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<ReceiverStream<Result<Event, Infallible>>>, AppError> {
    let access_token = bearer_token(&headers)?;
    let user_id = state.auth_service.current_user_id(&access_token).await?;
    let mut receiver = state.notification_service.subscribe();
    let (sender, stream) = mpsc::channel(64);

    tokio::spawn(async move {
        let mut heartbeat = time::interval(Duration::from_secs(25));

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    let event = Event::default()
                        .event("heartbeat")
                        .data(json!({ "ok": true }).to_string());

                    if sender.send(Ok(event)).await.is_err() {
                        break;
                    }
                }
                message = receiver.recv() => {
                    match message {
                        Ok(notification) if notification.user_id == user_id => {
                            let event = match Event::default()
                                .event("automation_alert")
                                .json_data(&notification.alert) {
                                Ok(event) => event,
                                Err(_) => continue,
                            };

                            if sender.send(Ok(event)).await.is_err() {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(stream)))
}
