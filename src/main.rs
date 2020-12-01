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

/// Retrieve GeoJSON data.
#[rocket::get("/geo/<name>/retrieve/json?<secret>&<from>&<to>&<max>")]
fn retrieve_json(
    db: DBConn,
    name: String,
    secret: Option<String>,
    from: Option<String>,
    to: Option<String>,
    max: Option<i64>,
) -> rocket::response::content::Json<String> {
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
    //println!("from {:?} to {:?}", from_ts, to_ts);
    //println!("secret {:?}", secret);

    let stmt = db.0.prepare_cached(
        r"SELECT t, lat, long, spd, ele FROM geohub.geodata
        WHERE (id = $1) and (t between $2 and $3) AND (secret = public.digest($4, 'sha256') or secret is null)
        LIMIT $5").unwrap(); // Must succeed.
    let rows = stmt
        .query(&[&name, &from_ts, &to_ts, &secret, &max])
        .unwrap();
    {
        println!("got {} rows", rows.len());
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

    rocket::response::content::Json(serde_json::to_string(&returnable).unwrap())
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
    let stmt = db.0.prepare_cached("INSERT INTO geohub.geodata (id, lat, long, spd, t, ele, secret) VALUES ($1, $2, $3, $4, $5, $6, public.digest($7, 'sha256'))").unwrap();
    stmt.execute(&[&name, &lat, &longitude, &s, &ts, &ele, &secret])
        .unwrap();
    rocket::http::Status::Ok
}

fn main() {
    rocket::ignite()
        .attach(DBConn::fairing())
        .mount("/", rocket::routes![log, retrieve_json])
        .launch();
}
