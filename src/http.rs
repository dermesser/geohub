use rocket::response::Responder;

#[derive(Responder)]
pub enum GeoHubResponse {
    #[response(status = 200, content_type = "json")]
    Ok(String),
    #[response(status = 400)]
    BadRequest(String),
    #[response(status = 500)]
    ServerError(String),
}

pub fn return_json<T: serde::Serialize>(obj: &T) -> GeoHubResponse {
    let json = serde_json::to_string(&obj);
    if let Ok(json) = json {
        return GeoHubResponse::Ok(json);
    } else {
        return GeoHubResponse::ServerError(json.unwrap_err().to_string());
    }
}

pub fn bad_request(msg: String) -> GeoHubResponse {
    GeoHubResponse::BadRequest(msg)
}

pub fn server_error(msg: String) -> GeoHubResponse {
    GeoHubResponse::ServerError(msg)
}
