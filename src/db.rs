use crate::types;

/// Queries for at most `limit` rows since entry ID `last`.
pub fn check_for_new_rows(
    db: &postgres::Connection,
    name: &str,
    secret: Option<&str>,
    last: &Option<i32>,
    limit: &Option<i64>,
) -> Option<(types::GeoJSON, i32)> {
    let mut returnable = types::GeoJSON {
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
                    .push(types::geofeature_from_row(ts, lat, long, spd, ele));
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

