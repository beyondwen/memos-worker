use super::*;

pub(crate) async fn public_route(
    req: &mut Request,
    env: &Env,
    path: &str,
    method: &Method,
) -> std::result::Result<Option<Response>, AppError> {
    if path == "/api/v1/instance" && matches!(method, &Method::Get) {
        return get_instance(env).await.map(Some);
    }
    if path == "/api/v1/setup" && matches!(method, &Method::Post) {
        return setup_admin(req, env).await.map(Some);
    }
    if path == "/api/v1/auth/signup" && matches!(method, &Method::Post) {
        return sign_up(req, env).await.map(Some);
    }
    if path == "/api/v1/auth/signin" && matches!(method, &Method::Post) {
        return sign_in(req, env).await.map(Some);
    }
    if path == "/api/v1/auth/refresh" && matches!(method, &Method::Post) {
        return refresh_session(req, env).await.map(Some);
    }
    if path == "/api/v1/auth/signout" && matches!(method, &Method::Post) {
        return sign_out().map(Some);
    }
    if path == "/api/v1/explore/rss.xml" && matches!(method, &Method::Get) {
        return generate_rss(env, None).await.map(Some);
    }
    if path.starts_with("/api/v1/u/")
        && path.ends_with("/rss.xml")
        && matches!(method, &Method::Get)
    {
        let username = path
            .trim_start_matches("/api/v1/u/")
            .trim_end_matches("/rss.xml")
            .trim_end_matches('/');
        return generate_rss(env, Some(username)).await.map(Some);
    }
    Ok(None)
}
