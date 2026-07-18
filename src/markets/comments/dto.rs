use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMarketCommentRequest {
    #[schema(example = "I think the probability is understated.", max_length = 2000)]
    pub body: String,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct MarketCommentsQuery {
    #[param(example = 50, minimum = 1, maximum = 100)]
    pub limit: Option<u64>,
    #[param(example = "96b5ce61-0c67-46a0-9925-bfbe3af0aa82")]
    pub before: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketCommentAuthorResponse {
    #[schema(example = "8c472518-9cfe-4c5b-bb7b-8da1be2aef4d")]
    pub id: String,
    #[schema(example = "uptions_user")]
    pub username: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketCommentResponse {
    #[schema(example = "96b5ce61-0c67-46a0-9925-bfbe3af0aa82")]
    pub id: String,
    #[schema(example = "123456")]
    pub market_id: String,
    pub author: MarketCommentAuthorResponse,
    #[schema(example = "I think the probability is understated.")]
    pub body: String,
    #[schema(example = "2026-07-18T12:30:00Z")]
    pub created_at: String,
    #[schema(example = "2026-07-18T12:30:00Z")]
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketCommentsPageResponse {
    pub comments: Vec<MarketCommentResponse>,
    #[schema(example = "96b5ce61-0c67-46a0-9925-bfbe3af0aa82")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct MarketCommentStreamEvent {
    #[schema(example = "market_comment.created")]
    pub event_type: String,
    pub comment: MarketCommentResponse,
}

#[cfg(test)]
mod tests {
    use super::{MarketCommentAuthorResponse, MarketCommentResponse, MarketCommentStreamEvent};

    #[test]
    fn public_comment_payload_contains_only_safe_author_fields() {
        let event = MarketCommentStreamEvent {
            event_type: "market_comment.created".to_owned(),
            comment: MarketCommentResponse {
                id: "comment-id".to_owned(),
                market_id: "market-id".to_owned(),
                author: MarketCommentAuthorResponse {
                    id: "user-id".to_owned(),
                    username: Some("alice".to_owned()),
                },
                body: "A useful observation".to_owned(),
                created_at: "2026-07-18T12:30:00Z".to_owned(),
                updated_at: "2026-07-18T12:30:00Z".to_owned(),
            },
        };

        let value = serde_json::to_value(event).unwrap();
        let author = value["comment"]["author"].as_object().unwrap();

        assert_eq!(author.len(), 2);
        assert_eq!(author["id"], "user-id");
        assert_eq!(author["username"], "alice");
        assert!(author.get("email").is_none());
        assert!(author.get("primary_wallet_address").is_none());
    }
}
