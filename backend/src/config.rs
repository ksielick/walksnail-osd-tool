use confy::ConfyError;
use derivative::Derivative;
use serde::{Deserialize, Serialize};

use crate::{ffmpeg::RenderSettings, osd::OsdOptions, srt::{SrtOptions, SrtType}, util::AppUpdate, NAMESPACE};

#[derive(Debug, Deserialize, Serialize, Derivative)]
#[derivative(Default)]
pub struct AppConfig {
    pub osd_options: OsdOptions,
    pub srt_options: SrtOptions,
    #[derivative(Default(value = "default_srt_profiles()"))]
    #[serde(default = "default_srt_profiles")]
    pub srt_profiles: std::collections::HashMap<SrtType, SrtOptions>,
    pub render_options: RenderSettings,
    pub app_update: AppUpdate,
    pub font_path: String,
    pub userfont_path: String,
    #[derivative(Default(value = "false"))]
    pub batch_processing: bool,
}

const CONFIG_NAME: &str = "saved_settings";

fn default_srt_profiles() -> std::collections::HashMap<SrtType, SrtOptions> {
    let mut map = std::collections::HashMap::new();
    map.insert(SrtType::Avatar, SrtOptions::walksnail_optimized());
    map.insert(SrtType::Ascent, SrtOptions::walksnail_optimized());
    map.insert(SrtType::AscentDebug, SrtOptions::walksnail_optimized());
    map.insert(SrtType::Artlynk, SrtOptions::default());
    map
}

impl AppConfig {
    #[tracing::instrument(ret)]
    pub fn load_or_create() -> Self {
        let config: Result<Self, _> = confy::load(NAMESPACE, CONFIG_NAME);
        if let Err(ConfyError::BadRonData(_)) = config {
            tracing::warn!("Invalid config found, resetting to default");
            let default_config = AppConfig::default();
            tracing::debug!("Default config: {:?}", default_config);
            default_config.save();
            default_config
        } else {
            config
                .map_err(|e| tracing::error!("Failed to load or create new config, caused by {e}"))
                .unwrap()
        }
    }

    #[tracing::instrument]
    pub fn save(&self) {
        confy::store(NAMESPACE, CONFIG_NAME, self)
            .map_err(|e| tracing::error!("Failed to save config file, {}", e))
            .ok();
    }
}
