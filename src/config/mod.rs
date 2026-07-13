use std::env;

use reqwest::Url;

const DEFAULT_PRODUCTION_ORIGIN: &str = "https://www.uptions.xyz";
const POLYMARKET_CLOB_HOST: &str = "clob.polymarket.com";
const POLYMARKET_GAMMA_HOST: &str = "gamma-api.polymarket.com";

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub server_address: String,
    pub database_url: String,
    pub credential_encryption_key: String,
    pub app_base_url: String,
    pub polymarket_clob_host: String,
    pub polymarket_gamma_host: String,
    pub environment: String,
    pub swagger_enabled: bool,
    pub cors_allowed_origins: Vec<String>,
    pub request_body_limit_bytes: usize,
    pub concurrency_limit: usize,
    pub public_rate_limit_per_minute: u32,
    pub auth_rate_limit_per_minute: u32,
    pub external_rate_limit_per_minute: u32,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let environment = env::var("APP_ENV").unwrap_or_else(|_| "development".to_owned());
        let production = is_production_environment(&environment);
        let polymarket_clob_host = env::var("POLYMARKET_CLOB_HOST")
            .unwrap_or_else(|_| format!("https://{POLYMARKET_CLOB_HOST}"));
        let polymarket_gamma_host = env::var("POLYMARKET_GAMMA_HOST")
            .unwrap_or_else(|_| format!("https://{POLYMARKET_GAMMA_HOST}"));

        if production {
            validate_polymarket_host(
                "POLYMARKET_CLOB_HOST",
                &polymarket_clob_host,
                POLYMARKET_CLOB_HOST,
            );
            validate_polymarket_host(
                "POLYMARKET_GAMMA_HOST",
                &polymarket_gamma_host,
                POLYMARKET_GAMMA_HOST,
            );
        }

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| DEFAULT_PRODUCTION_ORIGIN.to_owned())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(|origin| origin.trim_end_matches('/').to_owned())
            .collect();

        Self {
            server_address: env::var("SERVER_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_owned()),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            credential_encryption_key: env::var("CREDENTIAL_ENCRYPTION_KEY")
                .expect("CREDENTIAL_ENCRYPTION_KEY must be set"),
            app_base_url: env::var("APP_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_owned()),
            polymarket_clob_host: polymarket_clob_host.trim_end_matches('/').to_owned(),
            polymarket_gamma_host: polymarket_gamma_host.trim_end_matches('/').to_owned(),
            swagger_enabled: env_bool("SWAGGER_ENABLED", !production),
            cors_allowed_origins,
            request_body_limit_bytes: env_number("REQUEST_BODY_LIMIT_BYTES", 1_048_576),
            concurrency_limit: env_number("CONCURRENCY_LIMIT", 256),
            public_rate_limit_per_minute: env_number("PUBLIC_RATE_LIMIT_PER_MINUTE", 120),
            auth_rate_limit_per_minute: env_number("AUTH_RATE_LIMIT_PER_MINUTE", 10),
            external_rate_limit_per_minute: env_number("EXTERNAL_RATE_LIMIT_PER_MINUTE", 60),
            environment,
        }
    }

    pub fn is_production(&self) -> bool {
        is_production_environment(&self.environment)
    }
}

fn is_production_environment(environment: &str) -> bool {
    environment.eq_ignore_ascii_case("production") || environment.eq_ignore_ascii_case("prod")
}

fn env_bool(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) if value.eq_ignore_ascii_case("true") || value == "1" => true,
        Ok(value) if value.eq_ignore_ascii_case("false") || value == "0" => false,
        Ok(_) => panic!("{name} must be true, false, 1, or 0"),
        Err(_) => default,
    }
}

fn env_number<T>(name: &str, default: T) -> T
where
    T: Copy + Default + PartialOrd + std::str::FromStr,
{
    match env::var(name) {
        Ok(value) => {
            let parsed = value
                .parse()
                .unwrap_or_else(|_| panic!("{name} must be a valid positive number"));
            assert!(parsed > T::default(), "{name} must be greater than zero");
            parsed
        }
        Err(_) => default,
    }
}

fn validate_polymarket_host(name: &str, value: &str, allowed_host: &str) {
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

#[cfg(test)]
mod tests {
    use super::validate_polymarket_host;

    #[test]
    fn accepts_allowlisted_polymarket_host() {
        validate_polymarket_host(
            "POLYMARKET_CLOB_HOST",
            "https://clob.polymarket.com",
            "clob.polymarket.com",
        );
    }

    #[test]
    #[should_panic(expected = "must use https://clob.polymarket.com")]
    fn rejects_insecure_polymarket_host() {
        validate_polymarket_host(
            "POLYMARKET_CLOB_HOST",
            "http://clob.polymarket.com",
            "clob.polymarket.com",
        );
    }

    #[test]
    #[should_panic(expected = "must use https://clob.polymarket.com")]
    fn rejects_unlisted_polymarket_host() {
        validate_polymarket_host(
            "POLYMARKET_CLOB_HOST",
            "https://example.com",
            "clob.polymarket.com",
        );
    }
}
