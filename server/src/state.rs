use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, watch};

use crate::ai::{Job, ListCache};
use crate::db::Db;
use crate::events::Ev;
use crate::push::Vapid;
use crate::status::StatusMem;
use crate::weather::WeatherCache;

pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub media_dir: PathBuf,
    pub web_dir: PathBuf,
    /// `OPENROUTER_API_KEY` do ambiente (tem prioridade sobre a chave salva).
    pub env_openrouter_key: Option<String>,
    pub water_grid: Option<PathBuf>,
    pub tz_name: String,
}

impl Config {
    pub fn from_env() -> Config {
        fn env(name: &str) -> Option<String> {
            std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
        }
        let port = env("PORT").and_then(|p| p.parse().ok()).unwrap_or(8080);
        let data_dir = PathBuf::from(env("DATA_DIR").unwrap_or_else(|| "./data".into()));
        let media_dir = data_dir.join("media");
        Config {
            port,
            media_dir,
            data_dir,
            web_dir: PathBuf::from(env("WEB_DIR").unwrap_or_else(|| "./web/dist".into())),
            env_openrouter_key: env("OPENROUTER_API_KEY"),
            water_grid: env("WATER_GRID").map(PathBuf::from),
            tz_name: env("TZ_NAME").unwrap_or_else(|| "America/Sao_Paulo".into()),
        }
    }
}

pub struct Inner {
    pub cfg: Config,
    pub db: Db,
    pub events: broadcast::Sender<Ev>,
    /// Cliente HTTP geral (Open-Meteo, OpenRouter, Web Push).
    pub http: reqwest::Client,
    pub jobs: Mutex<HashMap<String, Job>>,
    pub lists: tokio::sync::Mutex<HashMap<&'static str, ListCache>>,
    pub weather: tokio::sync::Mutex<Option<WeatherCache>>,
    pub status: Mutex<StatusMem>,
    pub vapid: Vapid,
    pub shutdown: watch::Receiver<bool>,
}

pub type AppState = Arc<Inner>;
