/// Non-JSON plain point representation.
#[derive(Debug, Clone)]
pub struct GeoPoint {
    pub lat: f64,
    pub long: f64,
    pub spd: Option<f64>,
    pub ele: Option<f64>,
    pub time: chrono::DateTime<chrono::Utc>,
    pub note: Option<String>,
}

#[derive(serde::Serialize, Debug)]
pub struct LiveUpdate {
    #[serde(rename = "type")]
    typ: String, // always "GeoHubUpdate"
    last: Option<i32>,
    geo: Option<GeoJSON>,
    error: Option<String>,
}

impl LiveUpdate {
    pub fn new(last: Option<i32>, geo: Option<GeoJSON>, err: Option<String>) -> LiveUpdate {
        LiveUpdate {
            typ: "GeoHubUpdate".into(),
            last: last,
            geo: geo,
            error: err,
        }
    }
}

/// Fetch geodata as JSON.
///
#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoProperties {
    time: chrono::DateTime<chrono::Utc>,
    altitude: Option<f64>,
    speed: Option<f64>,
    /// The unique ID of the point.
    id: Option<i32>,
    /// An arbitrary note attached by the logging client.
    note: Option<String>,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoGeometry {
    #[serde(rename = "type")]
    typ: String, // always "Point"
    coordinates: (f64, f64), // always [long, lat]
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoFeature {
    #[serde(rename = "type")]
    typ: String, // always "Feature"
    properties: GeoProperties,
    geometry: GeoGeometry,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoJSON {
    #[serde(rename = "type")]
    typ: String, // always "FeatureCollection"
    features: Vec<GeoFeature>,
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

pub fn geofeature_from_point(id: Option<i32>, point: GeoPoint) -> GeoFeature {
    GeoFeature {
        typ: "Feature".into(),
        properties: GeoProperties {
            id: id,
            time: point.time,
            altitude: point.ele,
            speed: point.spd,
            note: point.note,
        },
        geometry: GeoGeometry {
            typ: "Point".into(),
            coordinates: (point.long, point.lat),
        },
    }
}
