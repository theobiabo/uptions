pub mod comments;
pub mod favorites;

use crate::error::AppError;

const MAX_MARKET_ID_LENGTH: usize = 128;

pub(crate) fn clean_market_id(market_id: &str) -> Result<String, AppError> {
    let market_id = market_id.trim();

    if market_id.is_empty()
        || market_id.chars().count() > MAX_MARKET_ID_LENGTH
        || market_id.chars().any(char::is_whitespace)
        || market_id.chars().any(char::is_control)
    {
        return Err(AppError::BadRequest("invalid market id".to_owned()));
    }

    Ok(market_id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::clean_market_id;

    #[test]
    fn validates_opaque_market_ids() {
        assert_eq!(clean_market_id(" 123456 ").unwrap(), "123456");
        assert_eq!(
            clean_market_id("0xabcdef0123456789").unwrap(),
            "0xabcdef0123456789"
        );
        assert!(clean_market_id("").is_err());
        assert!(clean_market_id("market id").is_err());
        assert!(clean_market_id(&"x".repeat(129)).is_err());
    }
}
