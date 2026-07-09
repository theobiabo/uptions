pub mod app;
pub mod auth;
pub mod automations;
pub mod config;
pub mod db;
pub mod entities;
pub mod error;
pub mod libs;
pub mod mcp;
pub mod notifications;
pub mod polymarket;
pub mod response;
pub mod users;

pub fn load_env() {
    let manifest_env = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");

    if manifest_env.exists() {
        if let Err(error) = dotenvy::from_path(manifest_env) {
            panic!("failed to load .env: {error}");
        }
    } else {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(error) if error.not_found() => {}
            Err(error) => {
                panic!("failed to load .env: {error}");
            }
        }
    }
}
