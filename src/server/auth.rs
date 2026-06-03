use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use subtle::ConstantTimeEq;

/// Token-based auth middleware. If token is empty, all requests are allowed.
pub async fn auth_middleware(
    expected_token: String,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // No token configured = no auth required
    if expected_token.is_empty() {
        return Ok(next.run(request).await);
    }

    // Check Authorization header: Bearer <token>
    if let Some(auth) = request.headers().get("authorization")
        && let Ok(val) = auth.to_str()
    {
        let expected = format!("Bearer {}", expected_token);
        if val.as_bytes().ct_eq(expected.as_bytes()).into() {
            return Ok(next.run(request).await);
        }
    }

    // Check query param: ?token=<token>
    if let Some(query) = request.uri().query() {
        for pair in query.split('&') {
            if let Some(token) = pair.strip_prefix("token=")
                && token.as_bytes().ct_eq(expected_token.as_bytes()).into()
            {
                return Ok(next.run(request).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}
