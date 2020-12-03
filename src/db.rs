use crate::ids;
use crate::types;

/// Managed by Rocket.
#[rocket_contrib::database("geohub")]
pub struct DBConn(postgres::Connection);

/// For requests from in- or outside a request handler.
pub struct DBQuery<'a>(pub &'a postgres::Connection);

impl<'a> DBQuery<'a> {
    /// Fetch records and format as JSON
    pub fn retrieve_json(
        &self,
        name: &str,
        from_ts: chrono::DateTime<chrono::Utc>,
        to_ts: chrono::DateTime<chrono::Utc>,
        secret: &str,
        limit: i64,
    ) -> Result<types::GeoJSON, postgres::Error> {
        let mut returnable = types::GeoJSON::new();
        let stmt = self.0.prepare_cached(
            r"SELECT t, lat, long, spd, ele FROM geohub.geodata
        WHERE (client = $1) and (t between $2 and $3) AND (secret = public.digest($4, 'sha256') or secret is null)
        ORDER BY t ASC
        LIMIT $5").unwrap(); // Must succeed.
        let rows = stmt.query(&[&name, &from_ts, &to_ts, &secret, &limit])?;
        returnable.reserve_features(rows.len());
        for row in rows.iter() {
            let (ts, lat, long, spd, ele) =
                (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4));
            returnable.push_feature(types::geofeature_from_row(ts, lat, long, spd, ele));
        }
        Ok(returnable)
    }

    pub fn log_geopoint(
        &self,
        name: &str,
        secret: &str,
        point: &types::GeoPoint,
    ) -> Result<(), postgres::Error> {
        let stmt = self.0.prepare_cached("INSERT INTO geohub.geodata (client, lat, long, spd, t, ele, secret) VALUES ($1, $2, $3, $4, $5, $6, public.digest($7, 'sha256'))").unwrap();
        let channel = format!("NOTIFY {}, '{}'", ids::channel_name(name, secret), name);
        let notify = self.0.prepare_cached(channel.as_str()).unwrap();
        stmt.execute(&[
            &name,
            &point.lat,
            &point.long,
            &point.spd,
            &point.time,
            &point.ele,
            &secret,
        ])
        .unwrap();
        notify.execute(&[]).unwrap();
        Ok(())
    }

    /// Queries for at most `limit` rows since entry ID `last`.
    pub fn check_for_new_rows(
        &self,
        name: &str,
        secret: Option<&str>,
        last: &Option<i32>,
        limit: &Option<i64>,
    ) -> Option<(types::GeoJSON, i32)> {
        let mut returnable = types::GeoJSON::new();
        let check_for_new = self.0.prepare_cached(
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
                returnable.reserve_features(rows.len());
                let mut last = 0;

                for row in rows.iter() {
                    let (id, ts, lat, long, spd, ele) = (
                        row.get(0),
                        row.get(1),
                        row.get(2),
                        row.get(3),
                        row.get(4),
                        row.get(5),
                    );
                    returnable.push_feature(types::geofeature_from_row(ts, lat, long, spd, ele));
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
}
