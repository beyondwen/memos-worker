#[derive(Debug)]
pub(crate) struct AppError {
    pub(crate) status: u16,
    pub(crate) message: String,
}

impl AppError {
    pub(crate) fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<worker::Error> for AppError {
    fn from(error: worker::Error) -> Self {
        Self::new(500, error.to_string())
    }
}
