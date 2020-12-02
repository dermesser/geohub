#![feature(proc_macro_hygiene, decl_macro)]

use postgres;
use rocket;

use chrono::TimeZone;

use fallible_iterator::FallibleIterator;
use std::iter::Iterator;

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

/// Fetch geodata as JSON.
///
#[derive(serde::Serialize, Debug)]
struct GeoProperties {
    time: chrono::DateTime<chrono::Utc>,
    altitude: Option<f64>,
    speed: Option<f64>,
}

#[derive(serde::Serialize, Debug)]
struct GeoGeometry {
    #[serde(rename = "type")]
    typ: String, // always "Point"
    coordinates: Vec<f64>, // always [long, lat]
}

#[derive(serde::Serialize, Debug)]
struct GeoFeature {
    #[serde(rename = "type")]
    typ: String, // always "Feature"
    properties: GeoProperties,
    geometry: GeoGeometry,
}

fn geofeature_from_row(
    ts: chrono::DateTime<chrono::Utc>,
    lat: Option<f64>,
    long: Option<f64>,
    spd: Option<f64>,
    ele: Option<f64>,
) -> GeoFeature {
    GeoFeature {
        typ: "Feature".into(),
        properties: GeoProperties {
            time: ts,
            altitude: ele,
            speed: spd,
        },
        geometry: GeoGeometry {
            typ: "Point".into(),
            coordinates: vec![long.unwrap_or(0.), lat.unwrap_or(0.)],
        },
    }
}

#[derive(serde::Serialize, Debug)]
struct GeoJSON {
    #[serde(rename = "type")]
    typ: String, // always "FeatureCollection"
    features: Vec<GeoFeature>,
}

#[derive(serde::Serialize, Debug)]
struct LiveUpdate {
    #[serde(rename = "type")]
    typ: String, // always "GeoHubUpdate"
    last: Option<i32>, // page token -- send in next request!
    geo: Option<GeoJSON>,
}

/// Queries for at most `limit` rows since entry ID `last`.
fn check_for_new_rows(
    db: &DBConn,
    name: &String,
    secret: &Option<String>,
    last: &Option<i32>,
    limit: &Option<i64>,
) -> Option<(GeoJSON, i32)> {
    let mut returnable = GeoJSON {
        typ: "FeatureCollection".into(),
        features: vec![],
    };
    let check_for_new = db.0.prepare_cached(
        r"SELECT id, t, lat, long, spd, ele FROM geohub.geodata
        WHERE (client = $1) and (id > $2) AND (secret = public.digest($3, 'sha256') or secret is null)
        ORDER BY id DESC
        LIMIT $4").unwrap(); // Must succeed.

    let last = last.unwrap_or(0);
    let limit = limit.unwrap_or(256);

    let rows = check_for_new.query(&[&name, &last, &secret, &limit]);
    if let Ok(rows) = rows {
        // If there are unknown entries, return those.
        if rows.len() > 0 {
            returnable.features = Vec::with_capacity(rows.len());
            let mut last = 0;

            for row in rows.iter() {
                let (id, ts, lat, long, spd, ele): (
                    i32,
                    chrono::DateTime<chrono::Utc>,
                    Option<f64>,
                    Option<f64>,
                    Option<f64>,
                    Option<f64>,
                ) = (
                    row.get(0),
                    row.get(1),
                    row.get(2),
                    row.get(3),
                    row.get(4),
                    row.get(5),
                );
                returnable
                    .features
                    .push(geofeature_from_row(ts, lat, long, spd, ele));
                if id > last {
                    last = id;
                }
            }

            return Some((returnable, last));
        }
        return None;
    } else {
        // For debugging.
        rows.unwrap();
    }
    return None;
}

/// Wait for an update.
///
/// Points are returned in descending order of time.
#[rocket::get("/geo/<name>/retrieve/live?<secret>&<last>&<timeout>")]
fn retrieve_live(
    db: DBConn,
    name: String,
    secret: Option<String>,
    last: Option<i32>,
    timeout: Option<i32>,
) -> rocket_contrib::json::Json<LiveUpdate> {
    // Only if the client supplied a paging token should we check for new rows before. This is an
    // optimization.
    if last.is_some() {
        if let Some((geojson, newlast)) = check_for_new_rows(&db, &name, &secret, &last, &None) {
            return rocket_contrib::json::Json(LiveUpdate {
                typ: "GeoHubUpdate".into(),
                last: Some(newlast),
                geo: Some(geojson),
            });
        }
    }

    // Otherwise we will wait for the next update.
    //
    let listen =
        db.0.prepare_cached(format!("LISTEN geohubclient_update_{}", name).as_str())
            .unwrap();
    let unlisten =
        db.0.prepare_cached(format!("UNLISTEN geohubclient_update_{}", name).as_str())
            .unwrap();

    listen.execute(&[]).ok();

    let timeout = std::time::Duration::new(timeout.unwrap_or(30) as u64, 0);
    if let Ok(_) = db.0.notifications().timeout_iter(timeout).next() {
        unlisten.execute(&[]).ok();
        if let Some((geojson, last)) = check_for_new_rows(&db, &name, &secret, &last, &Some(1)) {
            return rocket_contrib::json::Json(LiveUpdate {
                typ: "GeoHubUpdate".into(),
                last: Some(last),
                geo: Some(geojson),
            });
        }
    }
    unlisten.execute(&[]).ok();
    return rocket_contrib::json::Json(LiveUpdate {
        typ: "GeoHubUpdate".into(),
        last: last,
        geo: None,
    });
}

/// Retrieve GeoJSON data.
#[rocket::get("/geo/<name>/retrieve/json?<secret>&<from>&<to>&<max>")]
fn retrieve_json(
    db: DBConn,
    name: String,
    secret: Option<String>,
    from: Option<String>,
    to: Option<String>,
    max: Option<i64>,
) -> rocket_contrib::json::Json<GeoJSON> {
    let mut returnable = GeoJSON {
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
    let max = max.unwrap_or(16384);

    let stmt = db.0.prepare_cached(
        r"SELECT t, lat, long, spd, ele FROM geohub.geodata
        WHERE (client = $1) and (t between $2 and $3) AND (secret = public.digest($4, 'sha256') or secret is null)
        ORDER BY t ASC
        LIMIT $5").unwrap(); // Must succeed.
    let rows = stmt.query(&[&name, &from_ts, &to_ts, &secret, &max]);
    if let Ok(rows) = rows {
        returnable.features = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let (ts, lat, long, spd, ele): (
                chrono::DateTime<chrono::Utc>,
                Option<f64>,
                Option<f64>,
                Option<f64>,
                Option<f64>,
            ) = (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4));
            returnable
                .features
                .push(geofeature_from_row(ts, lat, long, spd, ele));
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
    if name.chars().any(|c| !c.is_alphanumeric()) {
        return rocket::http::Status::NotAcceptable;
    }
    let mut ts = chrono::Utc::now();
    if let Some(time) = time {
        ts = flexible_timestamp_parse(time).unwrap_or(ts);
    }
    let stmt = db.0.prepare_cached("INSERT INTO geohub.geodata (client, lat, long, spd, t, ele, secret) VALUES ($1, $2, $3, $4, $5, $6, public.digest($7, 'sha256'))").unwrap();
    let notify =
        db.0.prepare_cached(format!("NOTIFY geohubclient_update_{}, '{}'", name, name).as_str())
            .unwrap();
    stmt.execute(&[&name, &lat, &longitude, &s, &ts, &ele, &secret])
        .unwrap();
    notify.execute(&[]).unwrap();
    rocket::http::Status::Ok
}

/// Serve static files
#[rocket::get("/geo/assets/<file..>")]
fn assets(
    file: std::path::PathBuf,
) -> Result<rocket::response::NamedFile, rocket::response::status::NotFound<String>> {
    let p = std::path::Path::new("assets/").join(file);
    rocket::response::NamedFile::open(&p)
        .map_err(|e| rocket::response::status::NotFound(e.to_string()))
}

fn main() {
    rocket::ignite()
        .attach(DBConn::fairing())
        .mount(
            "/",
            rocket::routes![log, retrieve_json, retrieve_live, assets],
        )
        .launch();
}
