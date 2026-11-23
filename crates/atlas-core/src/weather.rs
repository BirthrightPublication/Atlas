//! Optional destination weather from a fixed, HTTPS-only provider.
//! Weather requests do not hold the Atlas database mutex. No precise street
//! address, mailbox contents, device position, or record ID is transmitted.
use chrono::Utc;
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use serde_json::{json, Value};
use std::{io::Read, time::Duration};

const MAX_RESPONSE: usize = 512 * 1024;
fn client() -> Result<Client, String> {
    Client::builder()
        .https_only(true)
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "Unable to initialize the weather service".into())
}
fn fetch_json(url: &str, parameters: &[(&str, &str)]) -> Result<Value, String> {
    // Fixed API endpoints: no user-controlled host, scheme, port or redirect.
    let response = client()?
        .get(url)
        .query(parameters)
        .send()
        .map_err(|_| "Weather service unavailable; check your connection".to_owned())?;
    if !response.status().is_success() {
        return Err(format!("Weather provider returned HTTP {}", response.status().as_u16()));
    }
    if response.content_length().is_some_and(|n| n > MAX_RESPONSE as u64) {
        return Err("Weather provider response exceeded the safety limit".into());
    }
    let mut bytes = Vec::new();
    response
        .take((MAX_RESPONSE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "Unable to read weather provider response".to_owned())?;
    if bytes.len() > MAX_RESPONSE {
        return Err("Weather provider response exceeded the safety limit".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "Weather provider returned invalid forecast data".into())
}
fn coordinates(payload: &Value) -> Result<(String, String), String> {
    let lat = payload["latitude"].as_f64().ok_or("Choose a destination with coordinates")?;
    let lon = payload["longitude"].as_f64().ok_or("Choose a destination with coordinates")?;
    if !lat.is_finite() || !lon.is_finite() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err("Destination coordinates are outside the valid range".into());
    }
    // Coordinates are rounded to ~1 km to avoid sending a precise property location.
    Ok((format!("{lat:.2}"), format!("{lon:.2}")))
}
pub fn query(method: &str, payload: &Value) -> Result<Value, String> {
    match method {
        "weather.search" => {
            let city = payload["city"].as_str().unwrap_or("").trim();
            if city.chars().count() < 2 || city.chars().count() > 80
                || city.chars().any(|c| c.is_control() || c.is_ascii_digit())
            {
                return Err("Enter a city or region name (not a street address)".into());
            }
            let response = fetch_json(
                "https://geocoding-api.open-meteo.com/v1/search",
                &[("name", city), ("count", "5"), ("language", "en"), ("format", "json")],
            )?;
            let cities = response["results"]
                .as_array()
                .map(|results| results.iter().filter_map(|result| {
                    let lat = result["latitude"].as_f64()?;
                    let lon = result["longitude"].as_f64()?;
                    if !lat.is_finite() || !lon.is_finite()
                        || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                        return None;
                    }
                    Some(json!({
                        "name":result["name"],"region":result["admin1"],
                        "country":result["country"],"latitude":lat,"longitude":lon,
                        "timezone":result["timezone"]
                    }))
                }).collect::<Vec<_>>())
                .unwrap_or_default();
            Ok(json!({"cities":cities,"source":"Open-Meteo Geocoding"}))
        }
        "weather.forecast" => {
            let (latitude, longitude) = coordinates(payload)?;
            let unit = payload["temperatureUnit"].as_str().unwrap_or("celsius");
            let (temperature_unit, wind_speed_unit) = match unit {
                "celsius" => ("celsius", "kmh"),
                "fahrenheit" => ("fahrenheit", "mph"),
                _ => return Err("Choose Celsius or Fahrenheit".into()),
            };
            let response = fetch_json(
                "https://api.open-meteo.com/v1/forecast",
                &[
                    ("latitude", &latitude),
                    ("longitude", &longitude),
                    ("timezone", "auto"),
                    ("forecast_days", "16"),
                    ("temperature_unit", temperature_unit),
                    ("wind_speed_unit", wind_speed_unit),
                    ("hourly", "temperature_2m,relative_humidity_2m,wind_speed_10m,wind_direction_10m,weather_code,precipitation_probability"),
                    ("daily", "weather_code,temperature_2m_max,temperature_2m_min,wind_speed_10m_max,precipitation_probability_max,sunrise,sunset"),
                ]
            )?;
            let dates = response["daily"]["time"].as_array();
            let hours = response["hourly"]["time"].as_array();
            if dates.is_none_or(|days| days.is_empty() || days.len() > 16)
                || hours.is_none_or(|items| items.is_empty() || items.len() > 16 * 24 + 24)
                || response["timezone"].as_str().is_none()
            {
                return Err("Forecast provider returned incomplete dates, hours, or timezone".into());
            }
            Ok(json!({
                "source":"Open-Meteo",
                "sourceUrl":"https://open-meteo.com/",
                "fetchedAt":Utc::now().to_rfc3339(),
                "forecastOnly":true,
                "timezone":response["timezone"],
                "latitude":latitude.parse::<f64>().unwrap_or_default(),
                "longitude":longitude.parse::<f64>().unwrap_or_default(),
                "daily":response["daily"],
                "dailyUnits":response["daily_units"],
                "hourly":response["hourly"],
                "hourlyUnits":response["hourly_units"]
            }))
        }
        _ => Err("Unsupported weather operation".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_precise_addresses_and_invalid_coordinates_without_network() {
        assert!(query("weather.search", &json!({"city":"123 Example Street"})).is_err());
        assert!(query("weather.search", &json!({"city":"A"})).is_err());
        assert!(coordinates(&json!({"latitude":91.0,"longitude":0.0})).is_err());
        assert!(coordinates(&json!({"latitude":51.23456,"longitude":-0.12345})).is_ok());
        assert_eq!(coordinates(&json!({"latitude":51.23456,"longitude":-0.12345})).unwrap(),("51.23".into(),"-0.12".into()));
        assert!(query("weather.other", &json!({})).is_err());
    }
}
