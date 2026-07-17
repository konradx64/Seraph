use maxminddb::Reader;
use std::net::IpAddr;

pub struct GeoIpService {
    reader: Option<Reader<Vec<u8>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeoLookupResult {
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl GeoIpService {
    pub fn new(db_path: Option<&str>) -> Self {
        let reader = db_path.and_then(|path| match Reader::open_readfile(path) {
            Ok(r) => {
                tracing::info!("Loaded MaxMind DB from: {}", path);
                Some(r)
            }
            Err(e) => {
                tracing::error!("Failed to open MaxMind DB at {}: {}", path, e);
                None
            }
        });
        Self { reader }
    }

    pub fn lookup(&self, ip: IpAddr) -> Option<GeoLookupResult> {
        let reader = self.reader.as_ref()?;

        #[derive(serde::Deserialize)]
        struct MaxMindRecord {
            country: Option<MaxMindCountry>,
            city: Option<MaxMindCity>,
            location: Option<MaxMindLocation>,
        }

        #[derive(serde::Deserialize)]
        struct MaxMindCountry {
            iso_code: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct MaxMindCity {
            names: Option<std::collections::HashMap<String, String>>,
        }

        #[derive(serde::Deserialize)]
        struct MaxMindLocation {
            latitude: Option<f64>,
            longitude: Option<f64>,
        }

        match reader.lookup(ip) {
            Ok(lookup_result) => match lookup_result.decode::<MaxMindRecord>() {
                Ok(Some(record)) => {
                    let country_code = record.country.and_then(|c| c.iso_code);
                    let city = record.city.and_then(|c| c.names).and_then(|mut names| {
                        names.remove("en").or_else(|| names.into_values().next())
                    });
                    let (latitude, longitude) = match record.location {
                        Some(loc) => (loc.latitude, loc.longitude),
                        None => (None, None),
                    };
                    Some(GeoLookupResult {
                        country_code,
                        city,
                        latitude,
                        longitude,
                    })
                }
                _ => None,
            },
            Err(_) => {
                // Address not found or query error
                None
            }
        }
    }
}
