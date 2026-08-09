use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Mutex;

use crate::domain::errors::DomainError;
use crate::domain::models::settings::TauriTavernSettings;
use crate::domain::models::update::{ReleaseInfo, UpdateCheckResult};
use crate::domain::repositories::settings_repository::SettingsRepository;
use crate::domain::repositories::update_repository::UpdateRepository;

struct CacheState {
    at: Instant,
    release: ReleaseInfo,
}

pub struct UpdateService {
    repository: Arc<dyn UpdateRepository>,
    settings_repository: Arc<dyn SettingsRepository>,
    cache: Mutex<Option<CacheState>>,
}

impl UpdateService {
    pub fn new(
        repository: Arc<dyn UpdateRepository>,
        settings_repository: Arc<dyn SettingsRepository>,
    ) -> Self {
        Self {
            repository,
            settings_repository,
            cache: Mutex::new(None),
        }
    }

    pub async fn check_for_update(&self) -> Result<UpdateCheckResult, DomainError> {
        let ttl_secs = self.read_cache_ttl_secs().await;

        if let Some(release) = self.try_read_cache(ttl_secs).await {
            return Ok(self.build_result(release));
        }

        // Cache miss (or expired, or TTL=0): hit repo.
        let result = self.repository.get_latest_release().await;
        match result {
            Ok(release) => {
                self.store_cache(release.clone()).await;
                Ok(self.build_result(release))
            }
            Err(err) => {
                // Do NOT cache errors; next call should retry.
                Err(err)
            }
        }
    }

    fn build_result(&self, latest_release: ReleaseInfo) -> UpdateCheckResult {
        let current_version = env!("CARGO_PKG_VERSION");
        let has_update = is_newer_version(current_version, &latest_release.version);

        UpdateCheckResult {
            has_update,
            current_version: current_version.to_string(),
            latest_release: if has_update {
                Some(latest_release)
            } else {
                None
            },
        }
    }

    async fn read_cache_ttl_secs(&self) -> u32 {
        match self.settings_repository.load_tauritavern_settings().await {
            Ok(TauriTavernSettings { updates, .. }) => updates.manual_check_cache_ttl_secs,
            Err(_) => 0, // 失败保守:不缓存,每次都打 repo。
        }
    }

    async fn try_read_cache(&self, ttl_secs: u32) -> Option<ReleaseInfo> {
        if ttl_secs == 0 {
            return None;
        }

        let guard = self.cache.lock().await;
        let state = guard.as_ref()?;
        if Instant::now().duration_since(state.at).as_secs() < ttl_secs as u64 {
            Some(state.release.clone())
        } else {
            None
        }
    }

    async fn store_cache(&self, release: ReleaseInfo) {
        let mut guard = self.cache.lock().await;
        *guard = Some(CacheState {
            at: Instant::now(),
            release,
        });
    }
}

fn is_newer_version(local: &str, remote: &str) -> bool {
    let parse = |value: &str| -> Vec<u64> {
        value
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    };

    let local_parts = parse(local);
    let remote_parts = parse(remote);

    for index in 0..local_parts.len().max(remote_parts.len()) {
        let left = local_parts.get(index).copied().unwrap_or(0);
        let right = remote_parts.get(index).copied().unwrap_or(0);

        if right > left {
            return true;
        }
        if right < left {
            return false;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::is_newer_version;

    #[test]
    fn newer_patch_version() {
        assert!(is_newer_version("1.3.0", "1.3.1"));
    }

    #[test]
    fn newer_minor_version() {
        assert!(is_newer_version("1.3.0", "1.4.0"));
    }

    #[test]
    fn newer_major_version() {
        assert!(is_newer_version("1.3.0", "2.0.0"));
    }

    #[test]
    fn same_version() {
        assert!(!is_newer_version("1.3.0", "1.3.0"));
    }

    #[test]
    fn older_version() {
        assert!(!is_newer_version("1.3.0", "1.2.9"));
    }

    #[test]
    fn different_segment_lengths() {
        assert!(is_newer_version("1.3", "1.3.1"));
        assert!(!is_newer_version("1.3.1", "1.3"));
    }

    use std::sync::Mutex as StdMutex;

    use crate::domain::errors::DomainError;
    use crate::domain::models::settings::{SettingsSnapshot, TauriTavernSettings, UserSettings};
    use crate::domain::models::update::ReleaseInfo;
    use crate::domain::repositories::settings_repository::SettingsRepository;
    use crate::domain::repositories::update_repository::UpdateRepository;

    struct CountingUpdateRepo {
        calls: StdMutex<usize>,
    }

    #[async_trait::async_trait]
    impl UpdateRepository for CountingUpdateRepo {
        async fn get_latest_release(&self) -> Result<ReleaseInfo, DomainError> {
            let mut c = self.calls.lock().unwrap();
            *c += 1;
            Ok(ReleaseInfo {
                tag_name: "v9.9.9".to_string(),
                version: "9.9.9".to_string(),
                name: "stub".to_string(),
                body: String::new(),
                html_url: "https://example.com".to_string(),
                prerelease: false,
                published_at: "2026-01-01T00:00:00Z".to_string(),
            })
        }
    }

    struct FixedSettingsRepo {
        ttl: u32,
    }

    #[async_trait::async_trait]
    impl SettingsRepository for FixedSettingsRepo {
        async fn save_tauritavern_settings(
            &self,
            _s: &TauriTavernSettings,
        ) -> Result<(), DomainError> {
            unreachable!()
        }
        async fn load_tauritavern_settings(&self) -> Result<TauriTavernSettings, DomainError> {
            let mut s = TauriTavernSettings::default();
            s.updates.manual_check_cache_ttl_secs = self.ttl;
            Ok(s)
        }

        async fn save_user_settings(&self, _settings: &UserSettings) -> Result<(), DomainError> {
            unreachable!()
        }
        async fn load_user_settings(&self) -> Result<UserSettings, DomainError> {
            unreachable!()
        }
        async fn create_snapshot(&self) -> Result<(), DomainError> {
            unreachable!()
        }
        async fn get_snapshots(&self) -> Result<Vec<SettingsSnapshot>, DomainError> {
            unreachable!()
        }
        async fn load_snapshot(&self, _name: &str) -> Result<UserSettings, DomainError> {
            unreachable!()
        }
        async fn restore_snapshot(&self, _name: &str) -> Result<(), DomainError> {
            unreachable!()
        }
        async fn get_themes(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!()
        }
        async fn get_moving_ui_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!()
        }
        async fn get_quick_reply_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!()
        }
        async fn get_instruct_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!()
        }
        async fn get_context_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!()
        }
        async fn get_sysprompt_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!()
        }
        async fn get_reasoning_presets(&self) -> Result<Vec<UserSettings>, DomainError> {
            unreachable!()
        }
        async fn get_koboldai_settings(&self) -> Result<(Vec<String>, Vec<String>), DomainError> {
            unreachable!()
        }
        async fn get_novelai_settings(&self) -> Result<(Vec<String>, Vec<String>), DomainError> {
            unreachable!()
        }
        async fn get_openai_settings(&self) -> Result<(Vec<String>, Vec<String>), DomainError> {
            unreachable!()
        }
        async fn get_textgen_settings(&self) -> Result<(Vec<String>, Vec<String>), DomainError> {
            unreachable!()
        }
        async fn get_world_names(&self) -> Result<Vec<String>, DomainError> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn cache_hits_within_ttl_window() {
        let repo = std::sync::Arc::new(CountingUpdateRepo {
            calls: StdMutex::new(0),
        });
        let settings_repo: std::sync::Arc<dyn SettingsRepository> =
            std::sync::Arc::new(FixedSettingsRepo { ttl: 1800 });
        let service = super::UpdateService::new(repo.clone(), settings_repo);

        let r1 = service.check_for_update().await.unwrap();
        let r2 = service.check_for_update().await.unwrap();
        let r3 = service.check_for_update().await.unwrap();

        assert!(r1.has_update, "first call should detect update");
        assert_eq!(r1.latest_release.as_ref().unwrap().version, "9.9.9");
        assert_eq!(r2.latest_release.as_ref().unwrap().version, "9.9.9");
        assert_eq!(r3.latest_release.as_ref().unwrap().version, "9.9.9");

        let count = *repo.calls.lock().unwrap();
        assert_eq!(
            count, 1,
            "subsequent calls within TTL must hit cache, repo should be called exactly once"
        );
    }

    #[tokio::test]
    async fn cache_bypassed_when_ttl_zero() {
        let repo = std::sync::Arc::new(CountingUpdateRepo {
            calls: StdMutex::new(0),
        });
        let settings_repo: std::sync::Arc<dyn SettingsRepository> =
            std::sync::Arc::new(FixedSettingsRepo { ttl: 0 });
        let service = super::UpdateService::new(repo.clone(), settings_repo);

        let _ = service.check_for_update().await.unwrap();
        let _ = service.check_for_update().await.unwrap();
        let _ = service.check_for_update().await.unwrap();

        let count = *repo.calls.lock().unwrap();
        assert_eq!(count, 3, "ttl=0 must skip cache and hit repo every call");
    }

    #[tokio::test]
    async fn cache_miss_after_repo_error_does_not_store() {
        struct FailingRepo;
        #[async_trait::async_trait]
        impl UpdateRepository for FailingRepo {
            async fn get_latest_release(&self) -> Result<ReleaseInfo, DomainError> {
                Err(DomainError::InternalError(
                    "simulated network failure".to_string(),
                ))
            }
        }
        let repo: std::sync::Arc<dyn UpdateRepository> = std::sync::Arc::new(FailingRepo);
        let settings_repo: std::sync::Arc<dyn SettingsRepository> =
            std::sync::Arc::new(FixedSettingsRepo { ttl: 1800 });
        let service = super::UpdateService::new(repo, settings_repo);

        let result = service.check_for_update().await;
        assert!(result.is_err(), "first call should propagate repo error");

        // Cache must NOT have stored the error; second call should still try repo (not return cached error).
        // Since FailingRepo always fails, we just verify it didn't short-circuit via a stored error.
        let result2 = service.check_for_update().await;
        assert!(result2.is_err());
    }
}
