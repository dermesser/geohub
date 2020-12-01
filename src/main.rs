#![feature(proc_macro_hygiene, decl_macro)]

use postgres;
use rocket;

use chrono::TimeZone;

#[rocket_contrib::database("geohub")]
struct DBConn(postgres::Connection);

/// lat, long are floats
/// time is like 2020-11-30T20:12:36.444Z (ISO 8601)
#[rocket::get("/geo/<name>/log?<lat>&<longitude>&<time>&<s>")]
fn hello(db: DBConn, name: String, lat: f64, longitude: f64, time: String, s: f64) -> &'static str {
    let ts = chrono::NaiveDateTime::parse_from_str(time.as_str(), "%Y-%m-%dT%H:%M:%S%.fZ")
        .ok()
        .map(|t| chrono::Utc::now().timezone().from_utc_datetime(&t));
    println!("{:?}", ts);
    db.0.execute(
        "INSERT INTO geohub.geodata (id, lat, long, spd, t) VALUES ($1, $2, $3, $4, $5)",
        &[&name, &lat, &longitude, &s, &ts],
    )
    .unwrap();
    "OK"
}

fn main() {
    rocket::ignite()
        .attach(DBConn::fairing())
        .mount("/", rocket::routes![hello])
        .launch();
}
