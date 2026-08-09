use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;

use crate::domain::errors::DomainError;
use crate::domain::models::secret::SecretKeys;
use crate::domain::models::update::ReleaseInfo;
use crate::domain::repositories::secret_repository::SecretRepository;
use crate::domain::repositories::update_repository::UpdateRepository;
use crate::infrastructure::github::classify_github_rate_limit;
use crate::infrastructure::http_client_pool::{HttpClientPool, HttpClientProfile};

const GITHUB_API_LATEST_RELEASE: &str =
    "https://api.github.com/repos/Darkatse/TauriTavern/releases/latest";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    prerelease: bool,
    published_at: Option<String>,
}

pub struct GitHubUpdateRepository {
    http_clients: Arc<HttpClientPool>,
    secret_repository: Arc<dyn SecretRepository>,
}

impl GitHubUpdateRepository {
    pub fn new(
        http_clients: Arc<HttpClientPool>,
        secret_repository: Arc<dyn SecretRepository>,
    ) -> Self {
        Self {
            http_clients,
            secret_repository,
        }
    }
}

#[async_trait]
impl UpdateRepository for GitHubUpdateRepository {
    async fn get_latest_release(&self) -> Result<ReleaseInfo, DomainError> {
        let client = self.http_clients.client(HttpClientProfile::Default)?;

        // PAT 软依赖:读取失败/不存在/空字符串都自动降级匿名调用,不抛错。
        // fail-fast 只在「真有 token 但请求仍失败」时触发(跟现状一致)。
        let token = self
            .secret_repository
            .read_secret(SecretKeys::GITHUB_TOKEN, None)
            .await
            .ok()
            .flatten()
            .filter(|s| !s.is_empty());

        let mut request = client
            .get(GITHUB_API_LATEST_RELEASE)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");

        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await.map_err(|error| {
            DomainError::InternalError(format!("GitHub API request failed: {error}"))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if let Some(domain_error) = classify_github_rate_limit(status, &body) {
                return Err(domain_error);
            }

            let snippet = body.trim();
            let suffix = if snippet.is_empty() {
                String::new()
            } else {
                format!(" ({snippet})")
            };

            return Err(DomainError::InternalError(format!(
                "GitHub API error: HTTP {}{}",
                status, suffix
            )));
        }

        let response: GitHubRelease = response.json().await.map_err(|error| {
            DomainError::InternalError(format!("Failed to parse GitHub response: {error}"))
        })?;

        let version = parse_version_from_tag(&response.tag_name);

        Ok(ReleaseInfo {
            tag_name: response.tag_name,
            version,
            name: response.name.unwrap_or_default(),
            body: response.body.unwrap_or_default(),
            html_url: response.html_url,
            prerelease: response.prerelease,
            published_at: response.published_at.unwrap_or_default(),
        })
    }
}

fn parse_version_from_tag(tag: &str) -> String {
    let tag = tag.trim();
    let Some(start) = tag.find(|c: char| c.is_ascii_digit()) else {
        return tag.to_string();
    };

    let candidate = &tag[start..];
    let end = candidate
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(candidate.len());
    candidate[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_version_from_tag;

    #[test]
    fn desktop_auto_tag() {
        assert_eq!(parse_version_from_tag("desktop-auto-v1.4.0"), "1.4.0");
    }

    #[test]
    fn simple_v_tag() {
        assert_eq!(parse_version_from_tag("v1.4.0"), "1.4.0");
    }

    #[test]
    fn bare_version() {
        assert_eq!(parse_version_from_tag("1.4.0"), "1.4.0");
    }

    #[test]
    fn mobile_tag() {
        assert_eq!(parse_version_from_tag("mobile-v2.0.0"), "2.0.0");
    }

    #[test]
    fn mobile_auto_tag() {
        assert_eq!(parse_version_from_tag("mobile-auto-v2.0.0"), "2.0.0");
    }

    #[test]
    fn suffix_is_stripped() {
        assert_eq!(parse_version_from_tag("v1.4.0-beta.1"), "1.4.0");
    }

    #[test]
    fn desktop_auto_branch_suffix_keeps_release_version() {
        assert_eq!(
            parse_version_from_tag("desktop-auto-v1.4.0-next-2.0.0"),
            "1.4.0"
        );
    }

    #[test]
    fn mobile_auto_branch_suffix_keeps_release_version() {
        assert_eq!(
            parse_version_from_tag("mobile-auto-v1.4.0-next-2.0.0"),
            "1.4.0"
        );
    }

    use crate::domain::models::secret::SecretKeys;
    use crate::domain::models::secret::Secrets;
    use crate::domain::repositories::secret_repository::SecretRepository;

    struct StubSecretRepo {
        token: Option<String>,
    }

    #[async_trait::async_trait]
    impl SecretRepository for StubSecretRepo {
        async fn save(&self, _secrets: &Secrets) -> Result<(), crate::domain::errors::DomainError> { unreachable!() }
        async fn load(&self) -> Result<Secrets, crate::domain::errors::DomainError> { unreachable!() }
        async fn clear_cache(&self) -> Result<(), crate::domain::errors::DomainError> { unreachable!() }
        async fn write_secret(&self, _key: &str, _value: &str, _label: &str) -> Result<String, crate::domain::errors::DomainError> { unreachable!() }
        async fn read_secret(&self, key: &str, _id: Option<&str>) -> Result<Option<String>, crate::domain::errors::DomainError> {
            assert_eq!(key, SecretKeys::GITHUB_TOKEN, "repo must ask for GITHUB_TOKEN, not {}", key);
            Ok(self.token.clone())
        }
        async fn delete_secret(&self, _key: &str, _id: Option<&str>) -> Result<(), crate::domain::errors::DomainError> { unreachable!() }
        async fn rotate_secret(&self, _key: &str, _id: &str) -> Result<(), crate::domain::errors::DomainError> { unreachable!() }
        async fn rename_secret(&self, _key: &str, _id: &str, _label: &str) -> Result<(), crate::domain::errors::DomainError> { unreachable!() }
    }

    #[test]
    fn constructor_accepts_secret_repository() {
        // 验证新签名不破坏构造(不真发请求)。
        let stub = StubSecretRepo { token: None };
        // HttpClientPool::new() 是 infallible(参 http_client_pool.rs:50)。
        let pool = std::sync::Arc::new(crate::infrastructure::http_client_pool::HttpClientPool::new());
        let _repo = super::GitHubUpdateRepository::new(pool, std::sync::Arc::new(stub));
    }
}
