use geo_types::Point;
use gpx::{self, Gpx};

/// Non-JSON plain point representation. Flat and representing a database row.
#[derive(Debug, Clone)]
pub struct GeoPoint {
    pub id: Option<i32>,
    pub lat: f64,
    pub long: f64,
    pub spd: Option<f64>, // in km/h by convention
    pub ele: Option<f64>,
    pub accuracy: Option<f64>,
    pub time: chrono::DateTime<chrono::Utc>,
    pub note: Option<String>,
}

impl GeoPoint {
    fn to_gpx_waypoint(self) -> gpx::Waypoint {
        let mut wp = gpx::Waypoint::new(Point::new(self.long, self.lat));
        wp.description = Some(format!("{}", self.id.unwrap_or(-1)));
        wp.elevation = self.ele;
        wp.speed = self.spd;
        wp.time = Some(self.time);
        wp.comment = self.note;
        wp.hdop = self.accuracy;
        wp
    }
}

/// Returned by the retrieve/live endpoint.
#[derive(serde::Serialize, Debug)]
pub struct LiveUpdate {
    #[serde(rename = "type")]
    typ: String, // always "GeoHubUpdate"
    client: String,
    last: Option<i32>,
    geo: Option<GeoJSON>,
    error: Option<String>,
}

impl LiveUpdate {
    pub fn new(
        client: String,
        last: Option<i32>,
        geo: Option<GeoJSON>,
        err: Option<String>,
    ) -> LiveUpdate {
        LiveUpdate {
            typ: "GeoHubUpdate".into(),
            client: client,
            last: last,
            geo: geo,
            error: err,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct LogLocations {
    pub locations: Vec<GeoFeature>,
}

/// Fetch geodata as JSON.
///
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct GeoProperties {
    #[serde(alias = "timestamp")]
    time: chrono::DateTime<chrono::Utc>,
    altitude: Option<f64>,
    speed: Option<f64>,
    #[serde(alias = "horizontal_accuracy")]
    accuracy: Option<f64>,
    /// The unique ID of the point.
    id: Option<i32>,
    /// An arbitrary note attached by the logging client.
    note: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct GeoGeometry {
    #[serde(rename = "type")]
    typ: String, // always "Point"
    coordinates: (f64, f64), // always [long, lat]
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct GeoFeature {
    #[serde(rename = "type")]
    typ: String, // always "Feature"
    properties: GeoProperties,
    geometry: GeoGeometry,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct GeoJSON {
    #[serde(rename = "type")]
    typ: String, // always "FeatureCollection"
    pub features: Vec<GeoFeature>,
}

impl GeoJSON {
    pub fn new() -> GeoJSON {
        GeoJSON {
            typ: "FeatureCollection".into(),
            features: vec![],
        }
    }
    pub fn reserve_features(&mut self, cap: usize) {
        self.features.reserve(cap);
    }
    pub fn push_feature(&mut self, feat: GeoFeature) {
        self.features.push(feat);
    }
}

pub fn geojson_from_points(points: Vec<GeoPoint>) -> GeoJSON {
    let mut gj = GeoJSON::new();
    gj.features = points.into_iter().map(geofeature_from_point).collect();
    return gj;
}

pub fn gpx_track_from_points(points: Vec<GeoPoint>) -> Gpx {
    let waypoints = points.into_iter().map(GeoPoint::to_gpx_waypoint).collect();

    let mut track_segment = gpx::TrackSegment::new();
    track_segment.points = waypoints;
    let mut track = gpx::Track::new();
    track.segments = vec![track_segment];
    let mut gx = Gpx::default();
    gx.tracks = vec![track];
    gx.version = gpx::GpxVersion::Gpx10;
    gx
}

pub fn geofeature_from_point(point: GeoPoint) -> GeoFeature {
    GeoFeature {
        typ: "Feature".into(),
        properties: GeoProperties {
            id: point.id,
            time: point.time,
            altitude: point.ele,
            speed: point.spd,
            note: point.note,
            accuracy: point.accuracy,
        },
        geometry: GeoGeometry {
            typ: "Point".into(),
            coordinates: (point.long, point.lat),
        },
    }
}

pub fn geopoint_from_feature(feat: GeoFeature) -> GeoPoint {
    let geo = feat.geometry;
    let prop = feat.properties;
    GeoPoint {
        id: prop.id,
        accuracy: prop.accuracy,
        ele: prop.altitude,
        long: geo.coordinates.0,
        lat: geo.coordinates.1,
        note: None,
        spd: prop.speed,
        time: prop.time,
    }
}
