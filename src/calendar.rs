use super::*;

const HOLIDAY_API_BASE: &str = "https://date.nager.at/api/v3";

pub(crate) async fn list_calendar_countries() -> std::result::Result<Response, AppError> {
    let url = format!("{}/AvailableCountries", HOLIDAY_API_BASE);
    let data = fetch_holiday_api(&url).await?;
    json_response(json!({ "countries": data }), 200).map_err(AppError::from)
}

pub(crate) async fn list_calendar_holidays(url: &Url) -> std::result::Result<Response, AppError> {
    let year = query_param(url, "year")
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|year| (1970..=2100).contains(year))
        .unwrap_or_else(current_year);
    let country = query_param(url, "country")
        .unwrap_or_else(|| "US".to_string())
        .trim()
        .to_ascii_uppercase();
    if !valid_country_code(&country) {
        return Err(AppError::new(400, "Invalid country code"));
    }
    let url = format!("{}/PublicHolidays/{}/{}", HOLIDAY_API_BASE, year, country);
    let data = fetch_holiday_api(&url).await?;
    json_response(
        json!({ "year": year, "country": country, "holidays": data }),
        200,
    )
    .map_err(AppError::from)
}

async fn fetch_holiday_api(url: &str) -> std::result::Result<Value, AppError> {
    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(url, &init)?;
    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() < 200 || response.status_code() >= 300 {
        return Err(AppError::new(
            502,
            format!("Holiday API returned HTTP {}", response.status_code()),
        ));
    }
    response.json().await.map_err(AppError::from)
}

pub(crate) fn valid_country_code(value: &str) -> bool {
    value.len() == 2 && value.chars().all(|ch| ch.is_ascii_uppercase())
}

fn current_year() -> i32 {
    let date = js_sys::Date::new_0();
    date.get_full_year() as i32
}
