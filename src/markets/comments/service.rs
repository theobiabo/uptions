use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Set,
};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    db::Db,
    entities::{market_comment, user},
    error::AppError,
    markets::{
        clean_market_id,
        comments::dto::{
            CreateMarketCommentRequest, MarketCommentAuthorResponse, MarketCommentResponse,
            MarketCommentStreamEvent, MarketCommentsPageResponse, MarketCommentsQuery,
        },
    },
};

const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 100;
const MAX_BODY_LENGTH: usize = 2000;

#[derive(Clone, Debug)]
pub(crate) struct PublishedMarketComment {
    pub market_id: String,
    pub event: MarketCommentStreamEvent,
}

#[derive(Clone)]
pub struct MarketCommentService {
    db: Db,
    sender: broadcast::Sender<PublishedMarketComment>,
}

impl MarketCommentService {
    pub fn new(db: Db) -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self { db, sender }
    }

    pub async fn list(
        &self,
        market_id: &str,
        query: MarketCommentsQuery,
    ) -> Result<MarketCommentsPageResponse, AppError> {
        let market_id = clean_market_id(market_id)?;
        let limit = page_size(query.limit)?;
        let mut comments_query =
            market_comment::Entity::find().filter(market_comment::Column::MarketId.eq(&market_id));

        if let Some(before) = clean_cursor(query.before.as_deref())? {
            let cursor = market_comment::Entity::find_by_id(&before)
                .filter(market_comment::Column::MarketId.eq(&market_id))
                .one(&self.db)
                .await?
                .ok_or_else(|| AppError::BadRequest("invalid comments cursor".to_owned()))?;
            comments_query = comments_query.filter(
                Condition::any()
                    .add(market_comment::Column::CreatedAt.lt(cursor.created_at))
                    .add(
                        Condition::all()
                            .add(market_comment::Column::CreatedAt.eq(cursor.created_at))
                            .add(market_comment::Column::Id.lt(cursor.id)),
                    ),
            );
        }

        let mut models = comments_query
            .order_by_desc(market_comment::Column::CreatedAt)
            .order_by_desc(market_comment::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;
        let has_more = models.len() as u64 > limit;
        models.truncate(limit as usize);
        let next_cursor = has_more
            .then(|| models.last().map(|comment| comment.id.clone()))
            .flatten();
        let authors = self.authors_for(&models).await?;
        let comments = models
            .into_iter()
            .map(|model| comment_response(model, &authors))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(MarketCommentsPageResponse {
            comments,
            next_cursor,
        })
    }

    pub async fn create(
        &self,
        market_id: &str,
        author_id: &str,
        payload: CreateMarketCommentRequest,
    ) -> Result<MarketCommentResponse, AppError> {
        let market_id = clean_market_id(market_id)?;
        let body = clean_body(&payload.body)?;
        let author = user::Entity::find_by_id(author_id)
            .one(&self.db)
            .await?
            .ok_or(AppError::Unauthorized)?;
        let now = Utc::now();
        let model = market_comment::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            market_id: Set(market_id.clone()),
            author_id: Set(author_id.to_owned()),
            body: Set(body),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;
        let response = comment_response(
            model,
            &HashMap::from([(
                author.id.clone(),
                MarketCommentAuthorResponse {
                    id: author.id,
                    username: author.username,
                },
            )]),
        )?;
        let _ = self.sender.send(PublishedMarketComment {
            market_id,
            event: MarketCommentStreamEvent {
                event_type: "market_comment.created".to_owned(),
                comment: response.clone(),
            },
        });

        Ok(response)
    }

    pub(crate) fn subscribe(
        &self,
        market_id: &str,
    ) -> Result<(String, broadcast::Receiver<PublishedMarketComment>), AppError> {
        Ok((clean_market_id(market_id)?, self.sender.subscribe()))
    }

    async fn authors_for(
        &self,
        comments: &[market_comment::Model],
    ) -> Result<HashMap<String, MarketCommentAuthorResponse>, AppError> {
        let author_ids = comments
            .iter()
            .map(|comment| comment.author_id.clone())
            .collect::<HashSet<_>>();

        if author_ids.is_empty() {
            return Ok(HashMap::new());
        }

        Ok(user::Entity::find()
            .filter(user::Column::Id.is_in(author_ids))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|author| {
                (
                    author.id.clone(),
                    MarketCommentAuthorResponse {
                        id: author.id,
                        username: author.username,
                    },
                )
            })
            .collect())
    }
}

fn comment_response(
    model: market_comment::Model,
    authors: &HashMap<String, MarketCommentAuthorResponse>,
) -> Result<MarketCommentResponse, AppError> {
    let author = authors.get(&model.author_id).cloned().ok_or_else(|| {
        AppError::DatabaseError("persisted market comment author was not found".to_owned())
    })?;

    Ok(MarketCommentResponse {
        id: model.id,
        market_id: model.market_id,
        author,
        body: model.body,
        created_at: model.created_at.to_rfc3339(),
        updated_at: model.updated_at.to_rfc3339(),
    })
}

fn clean_body(body: &str) -> Result<String, AppError> {
    let body = body.trim();
    let length = body.chars().count();

    if length == 0 {
        return Err(AppError::BadRequest(
            "comment body must not be empty".to_owned(),
        ));
    }
    if length > MAX_BODY_LENGTH {
        return Err(AppError::BadRequest(format!(
            "comment body must not exceed {MAX_BODY_LENGTH} characters"
        )));
    }

    Ok(body.to_owned())
}

fn clean_cursor(cursor: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let cursor = cursor.trim();
    let parsed = Uuid::parse_str(cursor)
        .map_err(|_| AppError::BadRequest("invalid comments cursor".to_owned()))?;

    Ok(Some(parsed.to_string()))
}

fn page_size(limit: Option<u64>) -> Result<u64, AppError> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);

    if !(1..=MAX_PAGE_SIZE).contains(&limit) {
        return Err(AppError::BadRequest(format!(
            "limit must be between 1 and {MAX_PAGE_SIZE}"
        )));
    }

    Ok(limit)
}

#[cfg(test)]
mod tests {
    use super::{MAX_BODY_LENGTH, clean_body, clean_cursor, page_size};

    #[test]
    fn trims_valid_comment_body() {
        assert_eq!(
            clean_body("  A useful observation  ").unwrap(),
            "A useful observation"
        );
    }

    #[test]
    fn rejects_empty_comment_body() {
        assert!(clean_body(" \n\t ").is_err());
    }

    #[test]
    fn enforces_comment_character_limit() {
        assert!(clean_body(&"é".repeat(MAX_BODY_LENGTH)).is_ok());
        assert!(clean_body(&"é".repeat(MAX_BODY_LENGTH + 1)).is_err());
    }

    #[test]
    fn validates_page_size_and_cursor() {
        assert_eq!(page_size(None).unwrap(), 50);
        assert_eq!(page_size(Some(100)).unwrap(), 100);
        assert!(page_size(Some(0)).is_err());
        assert!(page_size(Some(101)).is_err());
        assert!(clean_cursor(Some("not-a-uuid")).is_err());
        assert_eq!(
            clean_cursor(Some("96b5ce61-0c67-46a0-9925-bfbe3af0aa82")).unwrap(),
            Some("96b5ce61-0c67-46a0-9925-bfbe3af0aa82".to_owned())
        );
    }
}
