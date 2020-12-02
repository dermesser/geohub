#![feature(proc_macro_hygiene, decl_macro)]

mod db;
mod ids;
mod notifier;
mod types;

use std::sync::{mpsc, Arc, Mutex};
use std::time;

use postgres;
use rocket;

use chrono::TimeZone;

#[rocket_contrib::database("geohub")]
struct DBConn(postgres::Connection);

/// Parse timestamps flexibly. Without any zone information, UTC is assumed.
fn flexible_timestamp_parse(ts: String) -> Option<chrono::DateTime<chrono::Utc>> {
    let fmtstrings = &[
        "%Y-%m-%dT%H:%M:%S%.f%:z",
        "%Y-%m-%dT%H:%M:%S%.fZ",
        "%Y-%m-%d %H:%M:%S%.f",
    ];
    for fs in fmtstrings {
        let (naive, withtz) = (
            chrono::NaiveDateTime::parse_from_str(ts.as_str(), fs).ok(),
            chrono::DateTime::parse_from_str(ts.as_str(), fs).ok(),
        );
        if let Some(p) = withtz {
            return Some(p.with_timezone(&chrono::Utc));
        }
        if let Some(p) = naive {
            let utcd = chrono::Utc.from_utc_datetime(&p);
            return Some(utcd);
        }
    }
    None
}

#[derive(serde::Serialize, Debug)]
struct LiveUpdate {
    #[serde(rename = "type")]
    typ: String, // always "GeoHubUpdate"
    last: Option<i32>,
    geo: Option<types::GeoJSON>,
    error: Option<String>,
}

/// Almost like retrieve/json, but sorts in descending order and doesn't work with intervals (only
/// limit). Used for backfilling recent points in the UI.
#[rocket::get("/geo/<name>/retrieve/last?<secret>&<last>&<limit>")]
fn retrieve_last(
    db: DBConn,
    name: String,
    secret: Option<String>,
    last: Option<i32>,
    limit: Option<i64>,
) -> rocket_contrib::json::Json<LiveUpdate> {
    if let Some((geojson, newlast)) = db::check_for_new_rows(
        &db.0,
        &name,
        secret.as_ref().map(|s| s.as_str()),
        &last,
        &limit,
    ) {
        return rocket_contrib::json::Json(LiveUpdate {
            typ: "GeoHubUpdate".into(),
            last: Some(newlast),
            geo: Some(geojson),
            error: None,
        });
    }
    return rocket_contrib::json::Json(LiveUpdate {
        typ: "GeoHubUpdate".into(),
        last: last,
        geo: None,
        error: Some("No new rows returned".into()),
    });
}

/// Wait for an update.
/// Only one point is returned. To retrieve a history of points, call retrieve_last.
#[rocket::get("/geo/<name>/retrieve/live?<secret>&<timeout>")]
fn retrieve_live(
    notify_manager: rocket::State<notifier::SendableSender<notifier::NotifyRequest>>,
    name: String,
    secret: Option<String>,
    timeout: Option<u64>,
) -> rocket_contrib::json::Json<LiveUpdate> {
    if !ids::name_and_secret_acceptable(name.as_str(), secret.as_ref().map(|s| s.as_str())) {
        return rocket_contrib::json::Json(LiveUpdate {
            typ: "GeoHubUpdate".into(),
            last: None,
            geo: None,
            error: Some("You have supplied an invalid secret or name. Both must be ASCII alphanumeric strings.".into()),
        });
    }

    // Ask the notify thread to tell us when there is an update for this client name and secret.
    let (send, recv) = mpsc::channel();
    let send = notifier::SendableSender {
        sender: Arc::new(Mutex::new(send)),
    };

    let req = notifier::NotifyRequest {
        client: name.clone(),
        secret: secret,
        respond: send,
    };
    notify_manager.send(req).unwrap();

    if let Ok(response) = recv.recv_timeout(time::Duration::new(timeout.unwrap_or(30), 0)) {
        return rocket_contrib::json::Json(LiveUpdate {
            typ: "GeoHubUpdate".into(),
            last: response.last,
            geo: response.geo,
            error: None,
        });
    }
    return rocket_contrib::json::Json(LiveUpdate {
        typ: "GeoHubUpdate".into(),
        last: None,
        geo: None,
        error: Some("No new rows returned".into()),
    });
}

/// Retrieve GeoJSON data.
#[rocket::get("/geo/<name>/retrieve/json?<secret>&<from>&<to>&<limit>")]
fn retrieve_json(
    db: DBConn,
    name: String,
    secret: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
) -> rocket_contrib::json::Json<types::GeoJSON> {
    let mut returnable = types::GeoJSON {
        typ: "FeatureCollection".into(),
        features: vec![],
    };

    let from_ts = from
        .and_then(flexible_timestamp_parse)
        .unwrap_or(chrono::DateTime::from_utc(
            chrono::NaiveDateTime::from_timestamp(0, 0),
            chrono::Utc,
        ));
    let to_ts = to
        .and_then(flexible_timestamp_parse)
        .unwrap_or(chrono::Utc::now());
    let limit = limit.unwrap_or(16384);

    let stmt = db.0.prepare_cached(
        r"SELECT t, lat, long, spd, ele FROM geohub.geodata
        WHERE (client = $1) and (t between $2 and $3) AND (secret = public.digest($4, 'sha256') or secret is null)
        ORDER BY t ASC
        LIMIT $5").unwrap(); // Must succeed.
    let rows = stmt.query(&[&name, &from_ts, &to_ts, &secret, &limit]);
    if let Ok(rows) = rows {
        returnable.features = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let (ts, lat, long, spd, ele) =
                (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4));
            returnable
                .features
                .push(types::geofeature_from_row(ts, lat, long, spd, ele));
        }
    }

    rocket_contrib::json::Json(returnable)
}

/// Ingest geo data.

/// time is like 2020-11-30T20:12:36.444Z (ISO 8601). By default, server time is set.
/// secret can be used to protect points.
#[rocket::post("/geo/<name>/log?<lat>&<longitude>&<time>&<s>&<ele>&<secret>")]
fn log(
    db: DBConn,
    name: String,
    lat: f64,
    longitude: f64,
    secret: Option<String>,
    time: Option<String>,
    s: Option<f64>,
    ele: Option<f64>,
) -> rocket::http::Status {
    // Check that secret and client name are legal.
    if !ids::name_and_secret_acceptable(name.as_str(), secret.as_ref().map(|s| s.as_str())) {
        return rocket::http::Status::NotAcceptable;
    }
    let mut ts = chrono::Utc::now();
    if let Some(time) = time {
        ts = flexible_timestamp_parse(time).unwrap_or(ts);
    }
    let stmt = db.0.prepare_cached("INSERT INTO geohub.geodata (client, lat, long, spd, t, ele, secret) VALUES ($1, $2, $3, $4, $5, $6, public.digest($7, 'sha256'))").unwrap();
    let channel = format!(
        "NOTIFY {}, '{}'",
        ids::channel_name(name.as_str(), secret.as_ref().unwrap_or(&"".into())),
        name
    );
    let notify = db.0.prepare_cached(channel.as_str()).unwrap();
    stmt.execute(&[&name, &lat, &longitude, &s, &ts, &ele, &secret])
        .unwrap();
    notify.execute(&[]).unwrap();
    rocket::http::Status::Ok
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
    let send = notifier::SendableSender {
        sender: Arc::new(Mutex::new(send)),
    };

    rocket::ignite()
        .attach(DBConn::fairing())
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
