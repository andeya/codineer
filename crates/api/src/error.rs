use std::env::VarError;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(
        "missing {provider} credentials; export {} before calling the {provider} API",
        env_vars.join(" or ")
    )]
    MissingCredentials {
        provider: &'static str,
        env_vars: &'static [&'static str],
    },
    #[error("saved OAuth token is expired and no refresh token is available")]
    ExpiredOAuthToken,
    #[error("auth error: {0}")]
    Auth(String),
    #[error("failed to read credential environment variable: {0}")]
    InvalidApiKeyEnv(#[from] VarError),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{}", fmt_api_error(status, error_type, message, body, url))]
    Api {
        status: reqwest::StatusCode,
        error_type: Option<String>,
        message: Option<String>,
        body: String,
        url: Option<String>,
        retryable: bool,
    },
    #[error("api failed after {attempts} attempts: {last_error}")]
    RetriesExhausted {
        attempts: u32,
        #[source]
        last_error: Box<ApiError>,
    },
    #[error("invalid sse frame: {0}")]
    InvalidSseFrame(&'static str),
    #[error(
        "retry backoff overflowed on attempt {attempt} with base delay {base_delay:?}"
    )]
    BackoffOverflow {
        attempt: u32,
        base_delay: Duration,
    },
    #[error("response payload exceeded {limit} byte limit")]
    ResponsePayloadTooLarge { limit: usize },
    /// In-stream error object from the Anthropic Messages SSE protocol (`type: "error"`).
    #[error("{}", fmt_stream_app_error(error_type, message))]
    StreamApplicationError {
        error_type: Option<String>,
        message: String,
    },
}

fn fmt_api_error(
    status: &reqwest::StatusCode,
    error_type: &Option<String>,
    message: &Option<String>,
    body: &String,
    url: &Option<String>,
) -> String {
    let mut s = match (error_type.as_deref(), message.as_deref()) {
        (Some(et), Some(msg)) => format!("api returned {status} ({et}): {msg}"),
        _ if body.is_empty() => format!("api returned {status} (no response body)"),
        _ => format!("api returned {status}: {body}"),
    };
    if let Some(url) = url {
        use std::fmt::Write;
        let _ = write!(s, "\n  request url: {url}");
    }
    s
}

fn fmt_stream_app_error(error_type: &Option<String>, message: &String) -> String {
    match error_type.as_deref() {
        Some(t) => format!("stream error ({t}): {message}"),
        None => format!("stream error: {message}"),
    }
}

impl ApiError {
    #[must_use]
    pub const fn missing_credentials(
        provider: &'static str,
        env_vars: &'static [&'static str],
    ) -> Self {
        Self::MissingCredentials { provider, env_vars }
    }

    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(error) => error.is_connect() || error.is_timeout(),
            Self::Api { retryable, .. } => *retryable,
            Self::RetriesExhausted { last_error, .. } => last_error.is_retryable(),
            Self::MissingCredentials { .. }
            | Self::ExpiredOAuthToken
            | Self::Auth(_)
            | Self::InvalidApiKeyEnv(_)
            | Self::Io(_)
            | Self::Json(_)
            | Self::InvalidSseFrame(_)
            | Self::BackoffOverflow { .. }
            | Self::ResponsePayloadTooLarge { .. } => false,
            Self::StreamApplicationError { error_type, .. } => matches!(
                error_type.as_deref(),
                Some("overloaded_error" | "rate_limit_error")
            ),
        }
    }
}
