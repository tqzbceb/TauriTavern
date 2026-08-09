use serde_json::Value;
use std::sync::Arc;

use crate::application::dto::settings_dto::{
    SettingsSnapshotDto, SillyTavernSettingsResponseDto, TauriTavernSettingsDto,
    UpdateTauriTavernSettingsDto, UserSettingsDto,
};
use crate::application::errors::ApplicationError;
use crate::domain::models::settings::DevLoggingSettings;
use crate::domain::repositories::settings_repository::SettingsRepository;

pub struct SettingsService {
    settings_repository: Arc<dyn SettingsRepository>,
}

impl SettingsService {
    pub fn new(settings_repository: Arc<dyn SettingsRepository>) -> Self {
        Self {
            settings_repository,
        }
    }

    pub async fn get_tauritavern_settings(
        &self,
    ) -> Result<TauriTavernSettingsDto, ApplicationError> {
        tracing::debug!("Getting TauriTavern settings");

        let settings = self.settings_repository.load_tauritavern_settings().await?;

        Ok(TauriTavernSettingsDto::from(settings))
    }

    pub async fn update_tauritavern_settings(
        &self,
        dto: UpdateTauriTavernSettingsDto,
    ) -> Result<TauriTavernSettingsDto, ApplicationError> {
        tracing::debug!("Updating TauriTavern settings");

        let mut settings = self.settings_repository.load_tauritavern_settings().await?;

        if let Some(updates) = dto.updates {
            settings.updates.startup_popup.dismissed_release_token =
                updates.startup_popup.dismissed_release_token;
            if let Some(startup_check_enabled) = updates.startup_check_enabled {
                settings.updates.startup_check_enabled = startup_check_enabled;
            }
            if let Some(manual_check_cache_ttl_secs) = updates.manual_check_cache_ttl_secs {
                settings.updates.manual_check_cache_ttl_secs = manual_check_cache_ttl_secs;
            }
        }

        if let Some(perf_profile) = dto.perf_profile {
            settings.perf_profile = perf_profile;
        }

        if let Some(panel_runtime_profile) = dto.panel_runtime_profile {
            settings.panel_runtime_profile = panel_runtime_profile;
        }

        if let Some(embedded_runtime_profile) = dto.embedded_runtime_profile {
            settings.embedded_runtime_profile = embedded_runtime_profile;
        }

        if let Some(chat_history_mode) = dto.chat_history_mode {
            settings.chat_history_mode = chat_history_mode;
        }

        if let Some(close_to_tray_on_close) = dto.close_to_tray_on_close {
            settings.close_to_tray_on_close = close_to_tray_on_close;
        }

        if let Some(request_proxy) = dto.request_proxy {
            settings.request_proxy = request_proxy.into();
        }

        if let Some(allow_keys_exposure) = dto.allow_keys_exposure {
            settings.allow_keys_exposure = allow_keys_exposure;
        }

        if let Some(avatar_persona_original_images_enabled) =
            dto.avatar_persona_original_images_enabled
        {
            settings.avatar_persona_original_images_enabled =
                avatar_persona_original_images_enabled;
        }

        if let Some(dev) = dto.dev {
            if let Some(frontend_console_capture) = dev.frontend_console_capture {
                settings.dev.frontend_console_capture = frontend_console_capture;
            }

            if let Some(llm_api_keep) = dev.llm_api_keep {
                if !DevLoggingSettings::is_valid_llm_api_keep(llm_api_keep) {
                    return Err(ApplicationError::ValidationError(
                        "LLM API keep must be a positive number".to_string(),
                    ));
                }
                settings.dev.llm_api_keep = llm_api_keep;
            }
        }

        if let Some(dynamic_theme) = dto.dynamic_theme {
            if let Some(enabled) = dynamic_theme.enabled {
                settings.dynamic_theme.enabled = enabled;
            }

            if let Some(day_theme) = dynamic_theme.day_theme {
                settings.dynamic_theme.day_theme = day_theme;
            }

            if let Some(night_theme) = dynamic_theme.night_theme {
                settings.dynamic_theme.night_theme = night_theme;
            }

            if settings.dynamic_theme.enabled {
                if settings.dynamic_theme.day_theme.trim().is_empty() {
                    return Err(ApplicationError::ValidationError(
                        "Dynamic theme day theme is required".to_string(),
                    ));
                }

                if settings.dynamic_theme.night_theme.trim().is_empty() {
                    return Err(ApplicationError::ValidationError(
                        "Dynamic theme night theme is required".to_string(),
                    ));
                }
            }
        }

        if let Some(models) = dto.models {
            if let Some(claude) = models.claude {
                if let Some(prompt_cache_ttl) = claude.prompt_cache_ttl {
                    settings.models.claude.prompt_cache_ttl = prompt_cache_ttl;
                }
            }
        }

        self.settings_repository
            .save_tauritavern_settings(&settings)
            .await?;

        Ok(TauriTavernSettingsDto::from(settings))
    }

    pub async fn save_user_settings(
        &self,
        settings: UserSettingsDto,
    ) -> Result<(), ApplicationError> {
        tracing::info!("Saving user settings");

        let user_settings = settings.into();
        self.settings_repository
            .save_user_settings(&user_settings)
            .await?;

        Ok(())
    }

    pub async fn get_sillytavern_settings(
        &self,
    ) -> Result<SillyTavernSettingsResponseDto, ApplicationError> {
        tracing::info!("Getting SillyTavern settings");

        let user_settings = self.settings_repository.load_user_settings().await?;
        let settings_json = serde_json::to_string(&user_settings.data).map_err(|e| {
            ApplicationError::InternalError(format!("Failed to serialize settings: {}", e))
        })?;

        let (koboldai_settings, koboldai_setting_names) =
            self.settings_repository.get_koboldai_settings().await?;

        let (novelai_settings, novelai_setting_names) =
            self.settings_repository.get_novelai_settings().await?;

        let (openai_settings, openai_setting_names) =
            self.settings_repository.get_openai_settings().await?;

        let (textgen_settings, textgen_setting_names) =
            self.settings_repository.get_textgen_settings().await?;

        let world_names = self.settings_repository.get_world_names().await?;

        let themes = self.settings_repository.get_themes().await?;
        let themes_json: Vec<Value> = themes.into_iter().map(|t| t.data).collect();

        let moving_ui_presets = self.settings_repository.get_moving_ui_presets().await?;
        let moving_ui_presets_json: Vec<Value> =
            moving_ui_presets.into_iter().map(|p| p.data).collect();

        let quick_reply_presets = self.settings_repository.get_quick_reply_presets().await?;
        let quick_reply_presets_json: Vec<Value> =
            quick_reply_presets.into_iter().map(|p| p.data).collect();

        let instruct_presets = self.settings_repository.get_instruct_presets().await?;
        let instruct_presets_json: Vec<Value> =
            instruct_presets.into_iter().map(|p| p.data).collect();

        let context_presets = self.settings_repository.get_context_presets().await?;
        let context_presets_json: Vec<Value> =
            context_presets.into_iter().map(|p| p.data).collect();

        let sysprompt_presets = self.settings_repository.get_sysprompt_presets().await?;
        let sysprompt_presets_json: Vec<Value> =
            sysprompt_presets.into_iter().map(|p| p.data).collect();

        let reasoning_presets = self.settings_repository.get_reasoning_presets().await?;
        let reasoning_presets_json: Vec<Value> =
            reasoning_presets.into_iter().map(|p| p.data).collect();

        let response = SillyTavernSettingsResponseDto {
            settings: settings_json,
            koboldai_settings,
            koboldai_setting_names,
            world_names,
            novelai_settings,
            novelai_setting_names,
            openai_settings,
            openai_setting_names,
            textgenerationwebui_presets: textgen_settings,
            textgenerationwebui_preset_names: textgen_setting_names,
            themes: themes_json,
            moving_ui_presets: moving_ui_presets_json,
            quick_reply_presets: quick_reply_presets_json,
            instruct: instruct_presets_json,
            context: context_presets_json,
            sysprompt: sysprompt_presets_json,
            reasoning: reasoning_presets_json,
            enable_extensions: true,
            enable_extensions_auto_update: true,
            enable_accounts: false,
        };

        Ok(response)
    }

    pub async fn create_snapshot(&self) -> Result<(), ApplicationError> {
        tracing::info!("Creating settings snapshot");

        self.settings_repository.create_snapshot().await?;

        Ok(())
    }

    pub async fn get_snapshots(&self) -> Result<Vec<SettingsSnapshotDto>, ApplicationError> {
        tracing::info!("Getting settings snapshots");

        let snapshots = self.settings_repository.get_snapshots().await?;
        let snapshot_dtos = snapshots
            .into_iter()
            .map(SettingsSnapshotDto::from)
            .collect();

        Ok(snapshot_dtos)
    }

    pub async fn load_snapshot(&self, name: &str) -> Result<UserSettingsDto, ApplicationError> {
        tracing::info!("Loading settings snapshot: {}", name);

        let settings = self.settings_repository.load_snapshot(name).await?;

        Ok(UserSettingsDto::from(settings))
    }

    pub async fn restore_snapshot(&self, name: &str) -> Result<(), ApplicationError> {
        tracing::info!("Restoring settings snapshot: {}", name);

        self.settings_repository.restore_snapshot(name).await?;

        Ok(())
    }
}

#[cfg(test)]
mod update_patch_tests {
    use super::*;
    use crate::application::dto::settings_dto::{
        StartupUpdatePopupSettingsDto, TauriTavernUpdateSettingsDto, UpdateTauriTavernSettingsDto,
    };
    use crate::domain::errors::DomainError;
    use crate::domain::models::settings::{
        SettingsSnapshot, TauriTavernSettings, UserSettings,
    };
    use crate::domain::repositories::settings_repository::SettingsRepository;
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct StubSettingsRepo {
        saved: Mutex<Option<TauriTavernSettings>>,
    }

    #[async_trait]
    impl SettingsRepository for StubSettingsRepo {
        async fn save_tauritavern_settings(
            &self,
            settings: &TauriTavernSettings,
        ) -> Result<(), DomainError> {
            *self.saved.lock().unwrap() = Some(settings.clone());
            Ok(())
        }

        async fn load_tauritavern_settings(&self) -> Result<TauriTavernSettings, DomainError> {
            Ok(self
                .saved
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_default())
        }

        async fn save_user_settings(&self, _settings: &UserSettings) -> Result<(), DomainError> {
            unreachable!("save_user_settings not used in this test")
        }

        async fn load_user_settings(&self) -> Result<UserSettings, DomainError> {
            unreachable!("load_user_settings not used in this test")
        }

        async fn create_snapshot(&self) -> Result<(), DomainError> {
            unreachable!("create_snapshot not used in this test")
        }

        async fn get_snapshots(&self) -> Result<Vec<SettingsSnapshot>, DomainError> {
            unreachable!("get_snapshots not used in this test")
        }

        async fn load_snapshot(&self, _name: &str) -> Result<UserSettings, DomainError> {
            unreachable!("load_snapshot not used in this test")
        }

        async fn restore_snapshot(&self, _name: &str) -> Result<(), DomainError> {
            unreachable!("restore_snapshot not used in this test")
        }

        async fn get_themes(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!("get_themes not used in this test")
        }

        async fn get_moving_ui_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!("get_moving_ui_presets not used in this test")
        }

        async fn get_quick_reply_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!("get_quick_reply_presets not used in this test")
        }

        async fn get_instruct_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!("get_instruct_presets not used in this test")
        }

        async fn get_context_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!("get_context_presets not used in this test")
        }

        async fn get_sysprompt_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!("get_sysprompt_presets not used in this test")
        }

        async fn get_reasoning_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!("get_reasoning_presets not used in this test")
        }

        async fn get_koboldai_settings(&self) -> Result<(Vec<String>, Vec<String>), DomainError> {
            unreachable!("get_koboldai_settings not used in this test")
        }

        async fn get_novelai_settings(&self) -> Result<(Vec<String>, Vec<String>), DomainError> {
            unreachable!("get_novelai_settings not used in this test")
        }

        async fn get_openai_settings(&self) -> Result<(Vec<String>, Vec<String>), DomainError> {
            unreachable!("get_openai_settings not used in this test")
        }

        async fn get_textgen_settings(&self) -> Result<(Vec<String>, Vec<String>), DomainError> {
            unreachable!("get_textgen_settings not used in this test")
        }

        async fn get_world_names(&self) -> Result<Vec<String>, DomainError> {
            unreachable!("get_world_names not used in this test")
        }
    }

    #[tokio::test]
    async fn patch_transparently_propagates_new_update_fields() {
        let repo = std::sync::Arc::new(StubSettingsRepo {
            saved: Mutex::new(None),
        });
        let service = SettingsService::new(repo.clone());

        let dto = UpdateTauriTavernSettingsDto {
            updates: Some(TauriTavernUpdateSettingsDto {
                startup_popup: StartupUpdatePopupSettingsDto {
                    dismissed_release_token: None,
                },
                startup_check_enabled: Some(true),
                manual_check_cache_ttl_secs: Some(600),
            }),
            perf_profile: None,
            panel_runtime_profile: None,
            embedded_runtime_profile: None,
            chat_history_mode: None,
            close_to_tray_on_close: None,
            request_proxy: None,
            allow_keys_exposure: None,
            avatar_persona_original_images_enabled: None,
            dev: None,
            dynamic_theme: None,
            models: None,
        };

        let out = service.update_tauritavern_settings(dto).await.unwrap();
        assert_eq!(out.updates.startup_check_enabled, Some(true));
        assert_eq!(out.updates.manual_check_cache_ttl_secs, Some(600));

        let persisted = repo.saved.lock().unwrap().clone().unwrap();
        assert!(persisted.updates.startup_check_enabled);
        assert_eq!(persisted.updates.manual_check_cache_ttl_secs, 600);
    }
}
