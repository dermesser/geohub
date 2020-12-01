#![feature(proc_macro_hygiene, decl_macro)]

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

/// time is like 2020-11-30T20:12:36.444Z (ISO 8601). By default, server time is set.
/// secret can be used to protect points.
#[rocket::get("/geo/<name>/log?<lat>&<longitude>&<time>&<s>&<ele>&<secret>")]
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
    if name.chars().any(|c| !c.is_alphanumeric()) {
        return rocket::http::Status::NotAcceptable;
    }
    let mut ts = chrono::Utc::now();
    if let Some(time) = time {
        ts = flexible_timestamp_parse(time).unwrap_or(ts);
    }
    db.0.execute(
        "INSERT INTO geohub.geodata (id, lat, long, spd, t, ele, secret) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        &[&name, &lat, &longitude, &s, &ts, &ele, &secret],
    )
    .unwrap();
    rocket::http::Status::Ok
}

fn main() {
    rocket::ignite()
        .attach(DBConn::fairing())
        .mount("/", rocket::routes![log])
        .launch();
}
