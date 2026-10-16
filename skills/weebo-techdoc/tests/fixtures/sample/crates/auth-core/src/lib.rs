/// Checks a bearer token against the shared secret in `AUTH_SECRET`.
pub fn verify_token(token: &str) -> bool {
    match std::env::var("AUTH_SECRET") {
        Ok(secret) => !secret.is_empty() && token == secret,
        Err(_) => false,
    }
}
