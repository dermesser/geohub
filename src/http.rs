use rocket::response::Responder;

use std::io::Read;

#[derive(Responder)]
pub enum GeoHubResponse {
    #[response(status = 200, content_type = "plain")]
    Ok(String),
    #[response(status = 200, content_type = "json")]
    Json(String),
    #[response(status = 200, content_type = "application/xml")]
    Xml(String),
    #[response(status = 400)]
    BadRequest(String),
    #[response(status = 500)]
    ServerError(String),
}

pub fn return_xml(xml: String) -> GeoHubResponse {
    GeoHubResponse::Xml(xml)
}

pub fn return_json<T: serde::Serialize>(obj: &T) -> GeoHubResponse {
    let json = serde_json::to_string(&obj);
    if let Ok(json) = json {
        return GeoHubResponse::Json(json);
    } else {
        return GeoHubResponse::ServerError(json.unwrap_err().to_string());
    }
}

pub fn bad_request(msg: String) -> GeoHubResponse {
    GeoHubResponse::BadRequest(msg)
}

use std::fmt::Debug;

pub fn server_error<E: Debug>(err: E) -> GeoHubResponse {
    GeoHubResponse::ServerError(format!("{:?}", err))
}

pub fn read_data(d: rocket::Data, limit: u64) -> Result<String, GeoHubResponse> {
    let mut ds = d.open().take(limit);
    let mut dest = Vec::with_capacity(limit as usize);
    if let Err(e) = std::io::copy(&mut ds, &mut dest) {
        return Err(GeoHubResponse::BadRequest(format!(
            "Error reading request: {}",
            e
        )));
    }

    String::from_utf8(dest)
        .map_err(|e| GeoHubResponse::BadRequest(format!("Decoding error: {}", e)))
}
