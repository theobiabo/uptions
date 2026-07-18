use std::{convert::Infallible, time::Duration};

use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
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
    markets::comments::dto::{
        CreateMarketCommentRequest, MarketCommentResponse, MarketCommentStreamEvent,
        MarketCommentsPageResponse, MarketCommentsQuery,
    },
    response::{ApiResponse, created, ok},
};

#[utoipa::path(
    get,
    path = "/api/v1/markets/{market_id}/comments",
    tag = "Market Comments",
    security(("bearer_auth" = [])),
    params(
        ("market_id" = String, Path, description = "Market identifier"),
        MarketCommentsQuery
    ),
    responses(
        (status = 200, description = "Newest persisted market comments, ordered newest first", body = ApiResponse<MarketCommentsPageResponse>),
        (status = 400, description = "Invalid market id, cursor, or page size", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn list_market_comments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(market_id): Path<String>,
    Query(query): Query<MarketCommentsQuery>,
) -> Result<Json<ApiResponse<MarketCommentsPageResponse>>, AppError> {
    authenticated_user_id(&state, &headers).await?;
    let comments = state.market_comment_service.list(&market_id, query).await?;

    Ok(ok("Market comments fetched successfully", comments))
}

#[utoipa::path(
    post,
    path = "/api/v1/markets/{market_id}/comments",
    tag = "Market Comments",
    security(("bearer_auth" = [])),
    params(
        ("market_id" = String, Path, description = "Market identifier")
    ),
    request_body = CreateMarketCommentRequest,
    responses(
        (status = 201, description = "Market comment persisted", body = ApiResponse<MarketCommentResponse>),
        (status = 400, description = "Invalid market id or comment body", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn create_market_comment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(market_id): Path<String>,
    payload: Result<Json<CreateMarketCommentRequest>, JsonRejection>,
) -> Result<
    (
        axum::http::StatusCode,
        Json<ApiResponse<MarketCommentResponse>>,
    ),
    AppError,
> {
    let author_id = authenticated_user_id(&state, &headers).await?;
    let Json(payload) = payload.map_err(|error| {
        AppError::BadRequest(format!(
            "Invalid market comment payload: {}",
            error.body_text()
        ))
    })?;
    let comment = state
        .market_comment_service
        .create(&market_id, &author_id, payload)
        .await?;

    Ok(created("Market comment created successfully", comment))
}

#[utoipa::path(
    get,
    path = "/api/v1/markets/{market_id}/comments/stream",
    tag = "Market Comments",
    security(("bearer_auth" = [])),
    params(
        ("market_id" = String, Path, description = "Market identifier")
    ),
    responses(
        (status = 200, description = "Server-sent stream of newly persisted market comments", body = MarketCommentStreamEvent, content_type = "text/event-stream"),
        (status = 400, description = "Invalid market id", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn stream_market_comments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(market_id): Path<String>,
) -> Result<Sse<ReceiverStream<Result<Event, Infallible>>>, AppError> {
    authenticated_user_id(&state, &headers).await?;
    let (market_id, mut receiver) = state.market_comment_service.subscribe(&market_id)?;
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
                        Ok(published) if published.market_id == market_id => {
                            let comment_id = published.event.comment.id.clone();
                            let event = match Event::default()
                                .event("market_comment")
                                .id(comment_id)
                                .json_data(&published.event) {
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

async fn authenticated_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    let access_token = bearer_token(headers)?;
    state.auth_service.current_user_id(&access_token).await
}
