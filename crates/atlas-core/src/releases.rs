//! Published release information has no authority to install executable code.
use crate::{error, now, Atlas};
use semver::Version;
use serde_json::{json, Value};
use std::time::Duration;

pub fn normalize(input: &Value, current: &str) -> Result<Value, String> {
    let rows = input.as_array().ok_or("Release metadata must be a list")?;
    let current = Version::parse(current).map_err(|_| "Invalid application version")?;
    let mut releases = vec![];
    for row in rows.iter().take(100) {
        if row["draft"] == true {
            continue;
        }
        let tag = row["tag_name"].as_str().unwrap_or("");
        let Ok(version) = Version::parse(tag.trim_start_matches('v')) else {
            continue;
        };
        let Some(date) = row["published_at"].as_str() else {
            continue;
        };
        if chrono::DateTime::parse_from_rfc3339(date).is_err() {
            continue;
        }
        let notes = row["body"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(24000)
            .collect::<String>();
        let preview = row["prerelease"] == true || !version.pre.is_empty();
        releases.push((version.clone(),json!({"version":version.to_string(),"date":date,"summary":row["name"].as_str().unwrap_or(tag),"notes":notes,"channel":if preview{"preview"}else{"stable"},"current":version==current,"newer":version>current})));
    }
    releases.sort_by(|a, b| b.0.cmp(&a.0));
    releases.dedup_by(|a, b| a.0 == b.0);
    Ok(json!(releases
        .into_iter()
        .map(|(_, v)| v)
        .collect::<Vec<_>>()))
}
impl Atlas {
    pub(crate) fn release_history(&mut self, p: &Value) -> Result<Value, String> {
        let cached = self.settings()?["releaseHistoryCache"].clone();
        if p["refresh"] == false && cached.is_object() {
            return Ok(cached);
        }
        let fetched = (|| -> Result<Value, String> {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(12))
                .https_only(true)
                .user_agent("Birthright-Atlas-release-history")
                .build()
                .map_err(error)?;
            let response=client.get("https://api.github.com/repos/BirthrightPublication/Atlas/releases?per_page=100").header("Accept","application/vnd.github+json").send().map_err(|_|"Release history unavailable offline")?.error_for_status().map_err(|_|"Published release metadata could not be fetched")?;
            if response
                .content_length()
                .is_some_and(|n| n > 4 * 1024 * 1024)
            {
                return Err("Release metadata exceeds the size limit".into());
            }
            let mut bytes = vec![];
            std::io::Read::take(response, 4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(error)?;
            if bytes.len() > 4 * 1024 * 1024 {
                return Err("Release metadata exceeds the size limit".into());
            }
            let input: Value = serde_json::from_slice(&bytes).map_err(error)?;
            let releases = normalize(&input, env!("CARGO_PKG_VERSION"))?;
            Ok(
                json!({"releases":releases,"lastChecked":now(),"cached":false,"source":"GitHub published releases","installationAuthority":false}),
            )
        })();
        match fetched {
            Ok(value) => {
                self.save_settings(&json!({"releaseHistoryCache":value}))?;
                Ok(value)
            }
            Err(reason) => {
                let mut result = if cached.is_object() {
                    cached
                } else {
                    json!({"releases":[],"lastChecked":null})
                };
                result["cached"] = json!(true);
                result["error"] = json!(reason);
                Ok(result)
            }
        }
    }
}
use std::io::Read;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn published_versions_are_semantic_and_ci_artifacts_are_not_releases() {
        let v = normalize(
            &json!([
                {"tag_name":"v0.2.0","published_at":"2026-10-01T00:00:00Z"},
                {"tag_name":"v0.10.0","published_at":"2026-10-01T00:00:00Z"},
                {"tag_name":"v0.3.1-rc.1","published_at":"2026-10-01T00:00:00Z"},
                {"tag_name":"v9.0.0","draft":true,"published_at":"2026-10-01T00:00:00Z"},
                {"tag_name":"atlas-ci-123","published_at":"2026-10-01T00:00:00Z"}
            ]),
            "0.3.0",
        )
        .unwrap();
        assert_eq!(v.as_array().unwrap().len(), 3);
        assert_eq!(v[0]["version"], "0.10.0");
        assert_eq!(v[1]["channel"], "preview");
        assert_eq!(v[2]["newer"], false);
    }
}
