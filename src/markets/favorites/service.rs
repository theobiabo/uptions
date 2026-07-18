use chrono::Utc;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    sea_query::OnConflict,
};
use uuid::Uuid;

use crate::{
    db::Db,
    entities::market_favorite,
    error::AppError,
    markets::{
        clean_market_id,
        favorites::dto::{
            MarketFavoriteStatusResponse, MarketFavoritesPageResponse, MarketFavoritesQuery,
        },
    },
};

const DEFAULT_PAGE_SIZE: u64 = 50;
const MAX_PAGE_SIZE: u64 = 100;

#[derive(Clone)]
pub struct MarketFavoriteService {
    db: Db,
}

impl MarketFavoriteService {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn favorite(
        &self,
        user_id: &str,
        market_id: &str,
    ) -> Result<MarketFavoriteStatusResponse, AppError> {
        let market_id = clean_market_id(market_id)?;

        market_favorite::Entity::insert(market_favorite::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_owned()),
            market_id: Set(market_id.clone()),
            created_at: Set(Utc::now().into()),
        })
        .on_conflict(
            OnConflict::columns([
                market_favorite::Column::UserId,
                market_favorite::Column::MarketId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec(&self.db)
        .await?;

        Ok(MarketFavoriteStatusResponse {
            market_id,
            favorited: true,
        })
    }

    pub async fn unfavorite(
        &self,
        user_id: &str,
        market_id: &str,
    ) -> Result<MarketFavoriteStatusResponse, AppError> {
        let market_id = clean_market_id(market_id)?;

        market_favorite::Entity::delete_many()
            .filter(market_favorite::Column::UserId.eq(user_id))
            .filter(market_favorite::Column::MarketId.eq(&market_id))
            .exec(&self.db)
            .await?;

        Ok(MarketFavoriteStatusResponse {
            market_id,
            favorited: false,
        })
    }

    pub async fn status(
        &self,
        user_id: &str,
        market_id: &str,
    ) -> Result<MarketFavoriteStatusResponse, AppError> {
        let market_id = clean_market_id(market_id)?;
        let favorited = market_favorite::Entity::find()
            .filter(market_favorite::Column::UserId.eq(user_id))
            .filter(market_favorite::Column::MarketId.eq(&market_id))
            .one(&self.db)
            .await?
            .is_some();

        Ok(MarketFavoriteStatusResponse {
            market_id,
            favorited,
        })
    }

    pub async fn list(
        &self,
        user_id: &str,
        query: MarketFavoritesQuery,
    ) -> Result<MarketFavoritesPageResponse, AppError> {
        let limit = page_size(query.limit)?;
        let mut favorites_query =
            market_favorite::Entity::find().filter(market_favorite::Column::UserId.eq(user_id));

        if let Some(before) = clean_cursor(query.before.as_deref())? {
            let cursor = market_favorite::Entity::find_by_id(&before)
                .filter(market_favorite::Column::UserId.eq(user_id))
                .one(&self.db)
                .await?
                .ok_or_else(|| AppError::BadRequest("invalid favorites cursor".to_owned()))?;
            favorites_query = favorites_query.filter(
                Condition::any()
                    .add(market_favorite::Column::CreatedAt.lt(cursor.created_at))
                    .add(
                        Condition::all()
                            .add(market_favorite::Column::CreatedAt.eq(cursor.created_at))
                            .add(market_favorite::Column::Id.lt(cursor.id)),
                    ),
            );
        }

        let mut models = favorites_query
            .order_by_desc(market_favorite::Column::CreatedAt)
            .order_by_desc(market_favorite::Column::Id)
            .limit(limit + 1)
            .all(&self.db)
            .await?;
        let has_more = models.len() as u64 > limit;
        models.truncate(limit as usize);
        let next_cursor = has_more
            .then(|| models.last().map(|favorite| favorite.id.clone()))
            .flatten();
        let market_ids = models
            .into_iter()
            .map(|favorite| favorite.market_id)
            .collect();

        Ok(MarketFavoritesPageResponse {
            market_ids,
            next_cursor,
        })
    }
}

fn clean_cursor(cursor: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let cursor = cursor.trim();
    let parsed = Uuid::parse_str(cursor)
        .map_err(|_| AppError::BadRequest("invalid favorites cursor".to_owned()))?;

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
    use super::{clean_cursor, page_size};

    #[test]
    fn validates_favorites_page_size() {
        assert_eq!(page_size(None).unwrap(), 50);
        assert_eq!(page_size(Some(100)).unwrap(), 100);
        assert!(page_size(Some(0)).is_err());
        assert!(page_size(Some(101)).is_err());
    }

    #[test]
    fn validates_favorites_cursor() {
        assert!(clean_cursor(Some("not-a-uuid")).is_err());
        assert_eq!(
            clean_cursor(Some("96b5ce61-0c67-46a0-9925-bfbe3af0aa82")).unwrap(),
            Some("96b5ce61-0c67-46a0-9925-bfbe3af0aa82".to_owned())
        );
    }
}
