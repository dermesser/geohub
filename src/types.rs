
/// Fetch geodata as JSON.
///
#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoProperties {
    pub time: chrono::DateTime<chrono::Utc>,
    pub altitude: Option<f64>,
    pub speed: Option<f64>,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoGeometry {
    #[serde(rename = "type")]
    pub typ: String, // always "Point"
    pub coordinates: Vec<f64>, // always [long, lat]
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoFeature {
    #[serde(rename = "type")]
    pub typ: String, // always "Feature"
    pub properties: GeoProperties,
    pub geometry: GeoGeometry,
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

#[derive(serde::Serialize, Debug, Clone)]
pub struct GeoJSON {
    #[serde(rename = "type")]
    pub typ: String, // always "FeatureCollection"
    pub features: Vec<GeoFeature>,
}
