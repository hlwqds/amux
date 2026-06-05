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
        let expected = format!("Bearer {expected_token}");
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

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
    };
    use tower::util::ServiceExt;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn test_app(token: &str) -> Router {
        let token = token.to_string();
        Router::new()
            .route("/", get(ok_handler))
            .layer(middleware::from_fn(move |req, next| {
                let token = token.clone();
                super::auth_middleware(token, req, next)
            }))
    }

    #[tokio::test]
    async fn no_auth_header_returns_401() {
        let app = test_app("secret-token");
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn correct_bearer_token_passes() {
        let app = test_app("secret-token");
        let req = Request::builder()
            .uri("/")
            .header("authorization", "Bearer secret-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn wrong_bearer_token_returns_401() {
        let app = test_app("secret-token");
        let req = Request::builder()
            .uri("/")
            .header("authorization", "Bearer wrong-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
