use crate::error::ApiError;
use crate::providers::codineer_provider::{self, AuthSource, CodineerApiClient};
use crate::providers::openai_compat::{self, OpenAiCompatClient, OpenAiCompatConfig};
use crate::providers::{self, ProviderKind};
use crate::types::{MessageRequest, MessageResponse, StreamEvent};

#[derive(Debug, Clone)]
pub enum ProviderClient {
    CodineerApi(CodineerApiClient),
    Xai(OpenAiCompatClient),
    OpenAi(OpenAiCompatClient),
    Custom(OpenAiCompatClient),
}

impl ProviderClient {
    pub fn from_model(model: &str) -> Result<Self, ApiError> {
        Self::from_model_with_default_auth(model, None)
    }

    pub fn from_model_with_default_auth(
        model: &str,
        default_auth: Option<AuthSource>,
    ) -> Result<Self, ApiError> {
        let resolved_model = providers::resolve_model_alias(model);
        match providers::detect_provider_kind(&resolved_model) {
            ProviderKind::CodineerApi => Ok(Self::CodineerApi(match default_auth {
                Some(auth) => CodineerApiClient::from_auth(auth),
                None => CodineerApiClient::from_env()?,
            })),
            ProviderKind::Xai => Ok(Self::Xai(OpenAiCompatClient::from_env(
                OpenAiCompatConfig::xai(),
            )?)),
            ProviderKind::OpenAi => Ok(Self::OpenAi(OpenAiCompatClient::from_env(
                OpenAiCompatConfig::openai(),
            )?)),
            ProviderKind::Custom => Err(ApiError::Auth(
                "custom provider models must be resolved via from_custom()".to_string(),
            )),
        }
    }

    /// Build a provider client using a pre-resolved credential from a `CredentialChain`.
    pub fn from_model_with_credential(
        model: &str,
        credential: runtime::ResolvedCredential,
    ) -> Result<Self, ApiError> {
        let resolved_model = providers::resolve_model_alias(model);
        let auth = AuthSource::from(credential);
        match providers::detect_provider_kind(&resolved_model) {
            ProviderKind::CodineerApi => Ok(Self::CodineerApi(
                CodineerApiClient::from_auth(auth)
                    .with_base_url(codineer_provider::read_base_url()),
            )),
            ProviderKind::Xai => {
                let config = OpenAiCompatConfig::xai();
                Ok(Self::Xai(
                    OpenAiCompatClient::new(auth.api_key().unwrap_or_default(), config)
                        .with_base_url(openai_compat::read_base_url(config)),
                ))
            }
            ProviderKind::OpenAi => {
                let config = OpenAiCompatConfig::openai();
                Ok(Self::OpenAi(
                    OpenAiCompatClient::new(auth.api_key().unwrap_or_default(), config)
                        .with_base_url(openai_compat::read_base_url(config)),
                ))
            }
            ProviderKind::Custom => Err(ApiError::Auth(
                "custom provider models must be resolved via from_custom()".to_string(),
            )),
        }
    }

    /// Construct a `Custom` provider client from a pre-configured `OpenAiCompatClient`.
    #[must_use]
    pub fn from_custom(client: OpenAiCompatClient) -> Self {
        Self::Custom(client)
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        match self {
            Self::CodineerApi(_) => ProviderKind::CodineerApi,
            Self::Xai(_) => ProviderKind::Xai,
            Self::OpenAi(_) => ProviderKind::OpenAi,
            Self::Custom(_) => ProviderKind::Custom,
        }
    }

    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        match self {
            Self::CodineerApi(client) => client.send_message(request).await,
            Self::Xai(client) | Self::OpenAi(client) | Self::Custom(client) => {
                client.send_message(request).await
            }
        }
    }

    pub async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageStream, ApiError> {
        match self {
            Self::CodineerApi(client) => client
                .stream_message(request)
                .await
                .map(MessageStream::CodineerApi),
            Self::Xai(client) | Self::OpenAi(client) | Self::Custom(client) => client
                .stream_message(request)
                .await
                .map(MessageStream::OpenAiCompat),
        }
    }
}

#[derive(Debug)]
pub enum MessageStream {
    CodineerApi(codineer_provider::MessageStream),
    OpenAiCompat(openai_compat::MessageStream),
}

impl MessageStream {
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::CodineerApi(stream) => stream.request_id(),
            Self::OpenAiCompat(stream) => stream.request_id(),
        }
    }

    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        match self {
            Self::CodineerApi(stream) => stream.next_event().await,
            Self::OpenAiCompat(stream) => stream.next_event().await,
        }
    }
}

pub use codineer_provider::{
    oauth_token_is_expired, resolve_saved_oauth_token, resolve_startup_auth_source, OAuthTokenSet,
};
#[must_use]
pub fn read_base_url() -> String {
    codineer_provider::read_base_url()
}

#[must_use]
pub fn read_xai_base_url() -> String {
    openai_compat::read_base_url(OpenAiCompatConfig::xai())
}

#[cfg(test)]
mod tests {
    use crate::providers::{detect_provider_kind, resolve_model_alias, ProviderKind};

    #[test]
    fn resolves_existing_and_grok_aliases() {
        assert_eq!(resolve_model_alias("opus"), "claude-opus-4-6");
        assert_eq!(resolve_model_alias("grok"), "grok-3");
        assert_eq!(resolve_model_alias("grok-mini"), "grok-3-mini");
    }

    #[test]
    fn provider_detection_prefers_model_family() {
        assert_eq!(detect_provider_kind("grok-3"), ProviderKind::Xai);
        assert_eq!(
            detect_provider_kind("claude-sonnet-4-6"),
            ProviderKind::CodineerApi
        );
    }
}
