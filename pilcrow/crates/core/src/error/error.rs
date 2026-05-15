use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

/// Passed to the `handle_error` hook when an uncaught server error (5xx) occurs.
///
/// ```rust,ignore
/// // src/hooks.rs
/// pub async fn handle_error(error: &HookError, req: &Req) -> Option<Response> {
///     tracing::error!(status = error.status, "{}", error.message);
///     None  // let Pilcrow render its default 500 page
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HookError {
    /// HTTP status code of the error response (500, 502, 503, etc.).
    pub status: u16,
    /// Canonical reason phrase, e.g. `"Internal Server Error"`.
    pub message: String,
    /// Optional machine-readable source tag (e.g. the panic message), if captured.
    pub source: Option<String>,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("internal server error")]
    Internal,
    /// Redirect to another URL. Use this from `load()` to redirect before rendering.
    ///
    /// ```rust,ignore
    /// pub async fn load(ctx: PageContext) -> AppResult<Props> {
    ///     if !authenticated { return Err(AppError::redirect("/login")); }
    ///     Ok(Props { ... })
    /// }
    /// ```
    #[error("redirect to {0}")]
    Redirect(String),
}

impl AppError {
    /// Convenience constructor for `AppError::Redirect`.
    pub fn redirect(path: impl Into<String>) -> Self {
        AppError::Redirect(path.into())
    }

    /// HTTP status code that best represents this error.
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::NotFound(_) => 404,
            AppError::Unauthorized => 401,
            AppError::Validation(_) => 422,
            AppError::Internal => 500,
            AppError::Redirect(_) => 303,
        }
    }
}
