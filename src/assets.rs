use super::*;

pub(crate) async fn fetch_asset(
    req: &Request,
    env: &Env,
) -> std::result::Result<Response, AppError> {
    let assets = env.assets("ASSETS")?;
    let response = assets.fetch_request(req.clone()?).await?;
    if response.status_code() != 404 {
        return Ok(response);
    }
    let index_req = Request::new_with_init("/index.html", &RequestInit::new())?;
    Ok(assets.fetch_request(index_req).await?)
}
