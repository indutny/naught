extern crate hyper;
extern crate tokio_sync;

use std::fmt;
use std::error::Error as StdError;

#[derive(Debug)]
pub enum Error {
    Every,
}

impl StdError for Error {
    fn description(&self) -> &str {
        "TODO(indutny): implement me"
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Naught Error: {}", self.description())
    }
}

impl From<crate::node::Error> for Error {
    fn from(_: crate::node::Error) -> Self {
        Error::Every
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(_: std::net::AddrParseError) -> Self {
        Error::Every
    }
}

impl From<hyper::error::Error> for Error {
    fn from(_: hyper::error::Error) -> Self {
        Error::Every
    }
}

impl From<hyper::http::Error> for Error {
    fn from(_: hyper::http::Error) -> Self {
        Error::Every
    }
}

impl From<tokio_sync::mpsc::error::UnboundedRecvError> for Error {
    fn from(_: tokio_sync::mpsc::error::UnboundedRecvError) -> Self {
        Error::Every
    }
}

impl<T> From<tokio_sync::mpsc::error::UnboundedTrySendError<T>> for Error {
    fn from(_: tokio_sync::mpsc::error::UnboundedTrySendError<T>) -> Self {
        Error::Every
    }
}

impl From<tokio_sync::oneshot::error::RecvError> for Error {
    fn from(_: tokio_sync::oneshot::error::RecvError) -> Self {
        Error::Every
    }
}

impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Error::Every
    }
}
