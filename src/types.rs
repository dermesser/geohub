/// Non-JSON plain point representation.
#[derive(Debug, Clone)]
pub struct GeoPoint {
    pub lat: f64,
    pub long: f64,
    pub spd: Option<f64>,
    pub ele: Option<f64>,
    pub time: chrono::DateTime<chrono::Utc>,
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
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoGeometry {
    #[serde(rename = "type")]
    typ: String, // always "Point"
    coordinates: Vec<f64>, // always [long, lat]
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

pub fn geofeature_from_row(
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
