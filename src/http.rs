use rocket::response::Responder;

use std::io::Read;

#[derive(Responder)]
pub struct GeoHubResponder {
    inner: GeoHubResponse,
    cd: rocket::http::hyper::header::ContentDisposition,
}

#[derive(Responder)]
pub enum GeoHubResponse {
    #[response(status = 200, content_type = "plain")]
    Ok(String),
    #[response(status = 200, content_type = "json")]
    Json(String),
    #[response(status = 200, content_type = "application/gpx+xml")]
    Gpx(String),
    #[response(status = 400)]
    BadRequest(String),
    #[response(status = 500)]
    ServerError(String),
}

fn content_disposition(attachment: bool) -> rocket::http::hyper::header::ContentDisposition {
    rocket::http::hyper::header::ContentDisposition {
        disposition: if attachment {
            rocket::http::hyper::header::DispositionType::Attachment
        } else {
            rocket::http::hyper::header::DispositionType::Inline
        },
        parameters: vec![],
    }
}

pub fn return_ok(s: String) -> GeoHubResponder {
    let resp = GeoHubResponder {
        inner: GeoHubResponse::Ok(s),
        cd: content_disposition(false),
    };
    resp
}

pub fn return_gpx(gx: String) -> GeoHubResponder {
    let resp = GeoHubResponder {
        inner: GeoHubResponse::Gpx(gx),
        cd: content_disposition(true),
    };
    resp
}

pub fn return_json<T: serde::Serialize>(obj: &T) -> GeoHubResponder {
    let json = serde_json::to_string(&obj);
    let cd = content_disposition(true);
    if let Ok(json) = json {
        let resp = GeoHubResponder {
            inner: GeoHubResponse::Json(json),
            cd: cd,
        };
        return resp;
    } else {
        let resp = GeoHubResponder {
            inner: GeoHubResponse::ServerError(json.unwrap_err().to_string()),
            cd: cd,
        };
        return resp;
    }
}

pub fn bad_request(msg: String) -> GeoHubResponder {
    GeoHubResponder {
        inner: GeoHubResponse::BadRequest(msg),
        cd: content_disposition(false),
    }
}

use std::fmt::Debug;

pub fn server_error<E: Debug>(err: E) -> GeoHubResponder {
    GeoHubResponder {
        inner: GeoHubResponse::ServerError(format!("{:?}", err)),
        cd: content_disposition(false),
    }
}

pub fn read_data(d: rocket::Data, limit: u64) -> Result<String, GeoHubResponder> {
    let mut ds = d.open().take(limit);
    let mut dest = Vec::with_capacity(limit as usize);
    if let Err(e) = std::io::copy(&mut ds, &mut dest) {
        return Err(bad_request(format!("Error reading request: {}", e)));
    }

    String::from_utf8(dest).map_err(|e| bad_request(format!("Decoding error: {}", e)))
}
