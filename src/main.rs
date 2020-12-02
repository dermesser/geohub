#![feature(proc_macro_hygiene, decl_macro)]

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time;

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
    db: &postgres::Connection,
    name: &String,
    secret: &Option<String>,
    last: &Option<i32>,
    limit: &Option<i64>,
) -> Option<(GeoJSON, i32)> {
    let mut returnable = GeoJSON {
        typ: "FeatureCollection".into(),
        features: vec![],
    };
    let check_for_new = db.prepare_cached(
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
    if let Some((geojson, newlast)) = check_for_new_rows(&db.0, &name, &secret, &last, &limit) {
        return rocket_contrib::json::Json(LiveUpdate {
            typ: "GeoHubUpdate".into(),
            last: Some(newlast),
            geo: Some(geojson),
        });
    }
    return rocket_contrib::json::Json(LiveUpdate {
        typ: "GeoHubUpdate".into(),
        last: last,
        geo: None,
    });
}
/// Wait for an update.
/// Only one point is returned. To retrieve a history of points, call retrieve_last.
#[rocket::get("/geo/<name>/retrieve/live?<secret>&<timeout>")]
fn retrieve_live(
    notify_manager: rocket::State<SendableSender<NotifyRequest>>,
    name: String,
    secret: Option<String>,
    timeout: Option<u64>,
) -> rocket_contrib::json::Json<LiveUpdate> {
    let (send, recv) = mpsc::channel();
    let send = SendableSender {
        sender: Arc::new(Mutex::new(send)),
    };

    let req = NotifyRequest {
        client: name.clone(),
        secret: secret,
        respond: send,
    };
    notify_manager.send(req).unwrap();

    if let Ok(response) = recv.recv_timeout(time::Duration::new(timeout.unwrap_or(30), 0)) {
        eprintln!("Worker received response for {}", response.client);
        return rocket_contrib::json::Json(LiveUpdate {
            typ: "GeoHubUpdate".into(),
            last: response.last,
            geo: response.geo,
        });
    }
    return rocket_contrib::json::Json(LiveUpdate {
        typ: "GeoHubUpdate".into(),
        last: None,
        geo: None,
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

// Notify all waiters using just one DB connection.
struct NotifyRequest {
    client: String,
    secret: Option<String>,
    respond: SendableSender<NotifyResponse>,
}

struct NotifyResponse {
    client: String,
    // The GeoJSON object containing the update and the `last` page token.
    geo: Option<GeoJSON>,
    last: Option<i32>,
}

#[derive(Clone)]
struct SendableSender<T> {
    sender: Arc<Mutex<mpsc::Sender<T>>>,
}

impl<T> SendableSender<T> {
    fn send(&self, arg: T) -> Result<(), mpsc::SendError<T>> {
        let s = self.sender.lock().unwrap();
        s.send(arg)
    }
}

fn live_notifier_thread(rx: mpsc::Receiver<NotifyRequest>, db: postgres::Connection) {
    const TICK_MILLIS: u32 = 500;

    let mut clients: HashMap<String, Vec<NotifyRequest>> = HashMap::new();

    fn listen(db: &postgres::Connection, client: &str) -> postgres::Result<u64> {
        db.execute(&format!("LISTEN geohubclient_update_{}", client), &[])
    }
    fn unlisten(db: &postgres::Connection, client: &str) -> postgres::Result<u64> {
        db.execute(&format!("UNLISTEN geohubclient_update_{}", client), &[])
    }

    eprintln!("Notification thread running.");
    loop {
        // This loop checks for new messages on rx, then checks for new database notifications, etc.

        // Drain notification requests (clients asking to watch for notifications).
        loop {
            if let Ok(nrq) = rx.try_recv() {
                if !clients.contains_key(&nrq.client) {
                    listen(&db, &nrq.client).ok();
                }
                clients
                    .entry(nrq.client.clone())
                    .or_insert(vec![])
                    .push(nrq);
            } else {
                break;
            }
        }

        // Drain notifications from the database.
        // Also provide updated rows to the client.
        let notifications = db.notifications();
        let mut iter = notifications.timeout_iter(time::Duration::new(0, TICK_MILLIS * 1_000_000));
        let mut count = 0;
        while let Ok(Some(notification)) = iter.next() {
            let payload = notification.payload;
            unlisten(&db, &payload).ok();

            // One query per listening client as secrets may be different.
            // These queries use the primary key index returning one row only and will be quite fast.
            for request in clients.remove(&payload).unwrap_or(vec![]) {
                if let Some((geo, last)) =
                    check_for_new_rows(&db, &payload, &request.secret, &None, &Some(1))
                {
                    request
                        .respond
                        .send(NotifyResponse {
                            client: payload.clone(),
                            geo: Some(geo),
                            last: Some(last),
                        })
                        .ok();
                } else {
                    request
                        .respond
                        .send(NotifyResponse {
                            client: payload.clone(),
                            geo: None,
                            last: None,
                        })
                        .ok();
                }
            }

            // We also need to receive new notification requests.
            count += 1;
            if count > 3 {
                break;
            }
        }
    }
}

fn main() {
    let (send, recv) = mpsc::channel();
    let send = SendableSender {
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
                thread::spawn(move || live_notifier_thread(recv, conn));
                Ok(rocket)
            },
        ))
        .mount(
            "/",
            rocket::routes![log, retrieve_json, retrieve_last, retrieve_live, assets],
        )
        .launch();
}
