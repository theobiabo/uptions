use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};

use crate::{
    app::state::AppState,
    auth::handlers::bearer_token,
    error::{AppError, ErrorResponse},
    markets::favorites::dto::{
        MarketFavoriteStatusResponse, MarketFavoritesPageResponse, MarketFavoritesQuery,
    },
    providers::handlers::parse_provider,
    response::{ApiResponse, ok},
};

#[utoipa::path(
    put,
    path = "/api/v1/providers/{provider}/markets/{market_id}/favorite",
    tag = "Market Favorites",
    security(("bearer_auth" = [])),
    params(("provider" = String, Path), ("market_id" = String, Path)),
    responses(
        (status = 200, description = "Provider-scoped favorite status", body = ApiResponse<MarketFavoriteStatusResponse>),
        (status = 400, description = "Invalid provider or market", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn favorite_provider_market(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, market_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<MarketFavoriteStatusResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let provider = parse_provider(&provider)?;
    let favorite = state
        .market_favorite_service
        .favorite(provider, &user_id, &market_id)
        .await?;
    Ok(ok("Market favorited successfully", favorite))
}

#[utoipa::path(
    delete,
    path = "/api/v1/providers/{provider}/markets/{market_id}/favorite",
    tag = "Market Favorites",
    security(("bearer_auth" = [])),
    params(("provider" = String, Path), ("market_id" = String, Path)),
    responses(
        (status = 200, description = "Provider-scoped favorite status", body = ApiResponse<MarketFavoriteStatusResponse>),
        (status = 400, description = "Invalid provider or market", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn unfavorite_provider_market(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, market_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<MarketFavoriteStatusResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let provider = parse_provider(&provider)?;
    let favorite = state
        .market_favorite_service
        .unfavorite(provider, &user_id, &market_id)
        .await?;
    Ok(ok("Market unfavorited successfully", favorite))
}

#[utoipa::path(
    get,
    path = "/api/v1/providers/{provider}/markets/{market_id}/favorite",
    tag = "Market Favorites",
    security(("bearer_auth" = [])),
    params(("provider" = String, Path), ("market_id" = String, Path)),
    responses(
        (status = 200, description = "Provider-scoped favorite status", body = ApiResponse<MarketFavoriteStatusResponse>),
        (status = 400, description = "Invalid provider or market", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn get_provider_market_favorite_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((provider, market_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<MarketFavoriteStatusResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let provider = parse_provider(&provider)?;
    let favorite = state
        .market_favorite_service
        .status(provider, &user_id, &market_id)
        .await?;
    Ok(ok("Market favorite status fetched successfully", favorite))
}

#[utoipa::path(
    get,
    path = "/api/v1/providers/{provider}/markets/favorites",
    tag = "Market Favorites",
    security(("bearer_auth" = [])),
    params(("provider" = String, Path), MarketFavoritesQuery),
    responses(
        (status = 200, description = "Provider-scoped favorited market ids", body = ApiResponse<MarketFavoritesPageResponse>),
        (status = 400, description = "Invalid provider, cursor, or page size", body = ErrorResponse),
        (status = 401, description = "Missing or invalid bearer token", body = ErrorResponse)
    )
)]
pub async fn list_provider_market_favorites(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider): Path<String>,
    Query(query): Query<MarketFavoritesQuery>,
) -> Result<Json<ApiResponse<MarketFavoritesPageResponse>>, AppError> {
    let user_id = authenticated_user_id(&state, &headers).await?;
    let provider = parse_provider(&provider)?;
    let favorites = state
        .market_favorite_service
        .list(provider, &user_id, query)
        .await?;
    Ok(ok("Market favorites fetched successfully", favorites))
}

async fn authenticated_user_id(state: &AppState, headers: &HeaderMap) -> Result<String, AppError> {
    let access_token = bearer_token(headers)?;
    state.auth_service.current_user_id(&access_token).await
}
