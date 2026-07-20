use reqwest::Url;

pub const CLOB_HOST: &str = "clob.polymarket.com";
pub const GAMMA_HOST: &str = "gamma-api.polymarket.com";
pub const USER_WS_HOST: &str = "ws-subscriptions-clob.polymarket.com";

pub fn validate_api_host(name: &str, value: &str, allowed_host: &str) {
    let url = Url::parse(value).unwrap_or_else(|_| panic!("{name} must be a valid HTTPS URL"));
    let valid = url.scheme() == "https"
        && url.host_str() == Some(allowed_host)
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.path(), "" | "/")
        && url.query().is_none()
        && url.fragment().is_none();

    assert!(valid, "{name} must use https://{allowed_host}");
}

pub fn validate_user_ws_url(value: &str) {
    let url = Url::parse(value)
        .unwrap_or_else(|_| panic!("POLYMARKET_USER_WS_URL must be a valid WSS URL"));
    let valid = url.scheme() == "wss"
        && url.host_str() == Some(USER_WS_HOST)
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/ws/user"
        && url.query().is_none()
        && url.fragment().is_none();

    assert!(
        valid,
        "POLYMARKET_USER_WS_URL must use wss://{USER_WS_HOST}/ws/user"
    );
}

#[cfg(test)]
mod tests {
    use super::{CLOB_HOST, validate_api_host};

    #[test]
    fn accepts_allowlisted_polymarket_host() {
        validate_api_host(
            "POLYMARKET_CLOB_HOST",
            "https://clob.polymarket.com",
            CLOB_HOST,
        );
    }

    #[test]
    #[should_panic(expected = "must use https://clob.polymarket.com")]
    fn rejects_insecure_polymarket_host() {
        validate_api_host(
            "POLYMARKET_CLOB_HOST",
            "http://clob.polymarket.com",
            CLOB_HOST,
        );
    }

    #[test]
    #[should_panic(expected = "must use https://clob.polymarket.com")]
    fn rejects_unlisted_polymarket_host() {
        validate_api_host("POLYMARKET_CLOB_HOST", "https://example.com", CLOB_HOST);
    }
}
