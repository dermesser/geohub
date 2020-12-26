use chrono;

use chrono::TimeZone;

use crate::http;

/// Parse timestamps flexibly. Without any zone information, UTC is assumed.
pub fn flexible_timestamp_parse(ts: String) -> Option<chrono::DateTime<chrono::Utc>> {
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

pub fn to_kph(unit: &str, num: f64) -> Result<f64, http::GeoHubResponder> {
    match unit {
        "mps" | "ms" | "m/s" => Ok(3.6 * num),
        "kmh" | "km/h" | "kph" => Ok(num),
        "mph" => Ok(1.601 * num),
        "kn" | "knots" => Ok(1.852 * num),
        _ => Err(http::bad_request(format!("Unknown unit '{}'", unit))),
    }
}
