#![feature(proc_macro_hygiene, decl_macro)]

mod db;
mod http;
mod ids;
mod notifier;
mod types;
mod util;

use std::sync::{mpsc, Arc, Mutex};

use postgres;
use rocket;

/// Almost like retrieve/json, but sorts in descending order, doesn't work with intervals (only
/// limit), and returns a LiveUpdate.
/// Used for backfilling recent points in the UI.
#[rocket::get("/geo/<client>/retrieve/last?<secret>&<last>&<limit>")]
fn retrieve_last(
    db: db::DBConn,
    client: String,
    secret: Option<String>,
    last: Option<i32>,
    limit: Option<i64>,
) -> rocket_contrib::json::Json<types::LiveUpdate> {
    let secret = if let Some(secret) = secret {
        if secret.is_empty() {
            None
        } else {
            Some(secret)
        }
    } else {
        secret
    };
    let db = db::DBQuery(&db.0);
    if let Some((geojson, newlast)) = db.check_for_new_rows(&client, &secret, &last, &limit) {
        rocket_contrib::json::Json(types::LiveUpdate::new(client, Some(newlast), Some(geojson), None))
    } else {
        rocket_contrib::json::Json(types::LiveUpdate::new(
                client,
            last,
            None,
            Some("No rows returned".into()),
        ))
    }
}

/// Wait for an update.
/// Only one point is returned. To retrieve a history of points, call retrieve_last.
#[rocket::get("/geo/<name>/retrieve/live?<secret>&<timeout>")]
fn retrieve_live(
    notify_manager: rocket::State<notifier::NotifyManager>,
    name: String,
    secret: Option<String>,
    timeout: Option<u64>,
) -> http::GeoHubResponse {
    if !ids::name_and_secret_acceptable(name.as_str(), secret.as_ref().map(|s| s.as_str())) {
        return http::bad_request(
            "You have supplied an invalid secret or name. Both must be ASCII alphanumeric strings."
                .into(),
        );
    }
    let secret = if let Some(secret) = secret {
        if secret.is_empty() {
            None
        } else {
            Some(secret)
        }
    } else {
        secret
    };

    http::return_json(&notify_manager.wait_for_notification(name, secret, timeout))
}

/// Retrieve GeoJSON data.
#[rocket::get("/geo/<client>/retrieve/json?<secret>&<from>&<to>&<limit>&<last>")]
fn retrieve_json(
    db: db::DBConn,
    client: String,
    secret: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
    last: Option<i32>,
) -> http::GeoHubResponse {
    if !ids::name_and_secret_acceptable(client.as_str(), secret.as_ref().map(|s| s.as_str())) {
        return http::bad_request(
            "You have supplied an invalid secret or client. Both must be ASCII alphanumeric strings."
                .into(),
        );
    }
    let secret = if let Some(secret) = secret {
        if secret.is_empty() {
            None
        } else {
            Some(secret)
        }
    } else {
        secret
    };
    let db = db::DBQuery(&db.0);
    let from_ts =
        from.and_then(util::flexible_timestamp_parse)
            .unwrap_or(chrono::DateTime::from_utc(
                chrono::NaiveDateTime::from_timestamp(0, 0),
                chrono::Utc,
            ));
    let to_ts = to
        .and_then(util::flexible_timestamp_parse)
        .unwrap_or(chrono::Utc::now());
    let limit = limit.unwrap_or(16384);

    let result = db.retrieve_json(client.as_str(), from_ts, to_ts, &secret, limit, last);
    match result {
        Ok(json) => http::return_json(&json),
        Err(e) => http::server_error(e.to_string()),
    }
}

/// Ingest geo data.

/// time is like 2020-11-30T20:12:36.444Z (ISO 8601). By default, server time is set.
/// secret can be used to protect points.
#[rocket::post(
    "/geo/<name>/log?<lat>&<longitude>&<time>&<s>&<ele>&<secret>&<accuracy>",
    data = "<note>"
)]
fn log(
    db: db::DBConn,
    notify_manager: rocket::State<notifier::NotifyManager>,
    name: String,
    lat: f64,
    longitude: f64,
    secret: Option<String>,
    time: Option<String>,
    s: Option<f64>,
    ele: Option<f64>,
    accuracy: Option<f64>,
    note: rocket::data::Data,
) -> http::GeoHubResponse {
    let db = db::DBQuery(&db.0);
    // Check that secret and client name are legal.
    if !ids::name_and_secret_acceptable(name.as_str(), secret.as_ref().map(|s| s.as_str())) {
        return http::bad_request(
            "You have supplied an invalid secret or name. Both must be ASCII alphanumeric strings."
                .into(),
        );
    }
    let secret = if let Some(secret) = secret {
        if secret.is_empty() {
            None
        } else {
            Some(secret)
        }
    } else {
        secret
    };

    let mut ts = chrono::Utc::now();
    if let Some(time) = time {
        ts = util::flexible_timestamp_parse(time).unwrap_or(ts);
    }

    // Length-limit notes.
    let note = match http::read_data(note, 4096) {
        Ok(n) => { if n.is_empty() { None } else { Some(n) } },
        Err(e) => return e,
    };

    let point = types::GeoPoint {
        lat: lat,
        long: longitude,
        time: ts,
        spd: s,
        ele: ele,
        accuracy: accuracy,
        note: note,
    };
    if let Err(e) = db.log_geopoint(name.as_str(), &secret, &point) {
        return http::server_error(e.to_string());
    }
    if let Err(e) = notify_manager.send_notification(&db, name.as_str(), &secret) {
        eprintln!("Couldn't send notification: {}", e);
    }
    http::GeoHubResponse::Ok("".into())
}

/// Serve static files.
#[rocket::get("/geo/assets/<file..>")]
fn assets(
    file: std::path::PathBuf,
) -> Result<rocket::response::NamedFile, rocket::response::status::NotFound<String>> {
    let p = std::path::Path::new("assets/").join(file);
    rocket::response::NamedFile::open(&p)
        .map_err(|e| rocket::response::status::NotFound(e.to_string()))
}

fn main() {
    let (send, recv) = mpsc::channel();
    let send = notifier::NotifyManager(notifier::SendableSender {
        sender: Arc::new(Mutex::new(send)),
    });

    rocket::ignite()
        .attach(db::DBConn::fairing())
        .manage(send)
        .attach(rocket::fairing::AdHoc::on_attach(
            "Database Notifications",
            |rocket| {
                let dbconfig =
                    rocket_contrib::databases::database_config("geohub", &rocket.config()).unwrap();
                let url = dbconfig.url;
                let conn = postgres::Connection::connect(url, postgres::TlsMode::None).unwrap();
                std::thread::spawn(move || notifier::live_notifier_thread(recv, conn));
                Ok(rocket)
            },
        ))
        .mount(
            "/",
            rocket::routes![log, retrieve_json, retrieve_last, retrieve_live, assets],
        )
        .launch();
}
