use super::*;

pub(crate) async fn route(req: &mut Request, env: &Env) -> std::result::Result<Response, AppError> {
    let url = req.url().map_err(AppError::from)?;
    let path = url.path().to_string();
    let method = req.method();

    if method == Method::Options {
        return Ok(empty_response(204));
    }

    if let Some(response) = public_route(req, env, &path, &method).await? {
        return Ok(response);
    }

    if path.starts_with("/api/") || path.starts_with("/file/") {
        let viewer = current_viewer(req, env).await?;
        return authed_route(req, env, &url, &path, method, viewer).await;
    }

    fetch_asset(req, env).await
}
