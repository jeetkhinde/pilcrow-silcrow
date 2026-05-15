use pilcrow_core::AppError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("backend returned {status}: {body}")]
    Backend { status: u16, body: String },
}

impl From<ClientError> for pilcrow_core::AppError {
    fn from(err: ClientError) -> Self {
        match err {
            ClientError::Backend { status: 404, body } => AppError::NotFound(body),
            ClientError::Backend { status: 422, body } => AppError::Validation(body),
            ClientError::Backend { status: 401, .. } => AppError::Unauthorized,
            ClientError::Backend { .. } => AppError::Internal,
            ClientError::Http(_) => AppError::Internal,
        }
    }
}
