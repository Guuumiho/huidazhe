use super::*;

#[tauri::command]
pub(crate) fn load_settings(app: AppHandle) -> Result<Settings, String> {
    let path = crate::storage::settings_path(&app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }

    let contents = fs::read_to_string(&path).map_err(|error| format!("Failed to read settings: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("Failed to parse settings: {error}"))
}

#[tauri::command]
pub(crate) fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let config_dir = crate::storage::config_dir(&app)?;
    fs::create_dir_all(&config_dir).map_err(|error| format!("Failed to create config directory: {error}"))?;

    let sanitized = Settings {
        api_url: settings.api_url.trim().to_string(),
        api_key: settings.api_key.trim().to_string(),
        model: settings.model.trim().to_string(),
        theme: if settings.theme.trim().is_empty() {
            DEFAULT_THEME.to_string()
        } else {
            settings.theme.trim().to_string()
        },
        last_conversation_id: settings.last_conversation_id,
    };

    let contents =
        serde_json::to_string_pretty(&sanitized).map_err(|error| format!("Failed to serialize settings: {error}"))?;
    fs::write(crate::storage::settings_path(&app)?, contents)
        .map_err(|error| format!("Failed to save settings: {error}"))?;

    Ok(sanitized)
}
