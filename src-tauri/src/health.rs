use crate::paths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Status {
    Healthy,
    Degraded,
    Down,
    NotInstalled,
    WaitingPermissions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pipeline {
    #[serde(default)]
    pub uptime_secs: f64,
    #[serde(default)]
    pub capture_fps_actual: f64,
    #[serde(default)]
    pub frames_captured: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioPipeline {
    #[serde(default)]
    pub pending_transcription_segments: u64,
    #[serde(default)]
    pub total_words: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Health {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub audio_status: String,
    #[serde(default)]
    pub frame_status: String,
    #[serde(default)]
    pub monitors: Vec<String>,
    #[serde(default)]
    pub pipeline: Pipeline,
    #[serde(default)]
    pub audio_pipeline: AudioPipeline,
}

/// Pure: parse the /health JSON body.
pub fn parse(json: &str) -> Result<Health, serde_json::Error> {
    serde_json::from_str(json)
}

/// Pure: classify a parsed Health into a coarse Status.
pub fn classify(h: &Health) -> Status {
    match h.status.as_str() {
        "healthy" | "ok" => Status::Healthy,
        "degraded" => Status::Degraded,
        _ => Status::Down,
    }
}

/// Fetch + parse /health. Returns Down if unreachable.
pub async fn fetch() -> Status {
    let url = format!("http://127.0.0.1:{}/health", paths::PORT);
    match reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(body) => parse(&body).map(|h| classify(&h)).unwrap_or(Status::Down),
            Err(_) => Status::Down,
        },
        Err(_) => Status::Down,
    }
}

/// Full health object for the dashboard; None if unreachable/unparseable.
pub async fn fetch_full() -> Option<Health> {
    let url = format!("http://127.0.0.1:{}/health", paths::PORT);
    let resp = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .ok()?;
    let body = resp.text().await.ok()?;
    parse(&body).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed sample captured from the live instance (status degraded due to transcription backlog).
    const SAMPLE: &str = r#"{
        "status": "degraded",
        "version": "0.3.345",
        "message": "some systems are not healthy",
        "audio_status": "ok",
        "frame_status": "ok",
        "monitors": ["Display 1 (1470x956)"],
        "pipeline": {"uptime_secs": 2103.7, "capture_fps_actual": 0.084, "frames_captured": 178},
        "audio_pipeline": {"pending_transcription_segments": 3, "total_words": 2503}
    }"#;

    #[test]
    fn parses_live_sample() {
        let h = parse(SAMPLE).unwrap();
        assert_eq!(h.version, "0.3.345");
        assert_eq!(h.monitors.len(), 1);
        assert_eq!(h.audio_pipeline.pending_transcription_segments, 3);
        assert_eq!(h.pipeline.frames_captured, 178);
    }

    #[test]
    fn classify_maps_status_strings() {
        let mut h = Health::default();
        h.status = "degraded".into();
        assert_eq!(classify(&h), Status::Degraded);
        h.status = "healthy".into();
        assert_eq!(classify(&h), Status::Healthy);
        h.status = "whatever".into();
        assert_eq!(classify(&h), Status::Down);
    }

    #[test]
    fn missing_fields_default_gracefully() {
        let h = parse("{}").unwrap();
        assert_eq!(h.version, "");
        assert_eq!(h.pipeline.frames_captured, 0);
    }
}
