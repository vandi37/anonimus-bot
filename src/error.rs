use redis::RedisError;
use teloxide::RequestError;

#[derive(Debug)]
pub enum Error {
    RequestError(RequestError),
    Serde(serde_json::Error),
    Redis(RedisError),
}

impl From<RequestError> for Error {
    fn from(err: RequestError) -> Self {
        Error::RequestError(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serde(err)
    }
}

impl From<RedisError> for Error {
    fn from(err: RedisError) -> Self {
        Error::Redis(err)
    }
}