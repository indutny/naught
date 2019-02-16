extern crate hyper;
extern crate serde;
extern crate tokio;

use std::error::Error as StdError;
use std::fmt;

use serde::Serialize;

#[derive(Serialize, Debug)]
pub enum Error {
    Node(crate::node::Error),
    AddrParse(String),
    Hyper(String),
    HyperHTTP(String),
    MPSCRecv,
    MPSCSend(String),
    MPSCTrySend,
    OneShotRecv,
    OneShotSend,
    TimerError,
    Unreachable,
    JSON(String),
}

impl StdError for Error {
    fn description(&self) -> &str {
        "TODO(indutny): implement me"
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Node(err) => write!(f, "Node: {}", err.description()),
            Error::AddrParse(s) => write!(f, "AddrParse: {}", s),
            Error::Hyper(s) => write!(f, "Hyper: {}", s),
            Error::HyperHTTP(s) => write!(f, "Hyper HTTP: {}", s),
            Error::MPSCRecv => write!(f, "MPSCRecv"),
            Error::MPSCSend(s) => write!(f, "MPSCSend: {}", s),
            Error::MPSCTrySend => write!(f, "MPSCTrySend"),
            Error::OneShotRecv => write!(f, "OneShotRecv"),
            Error::OneShotSend => write!(f, "OneShotSend"),
            Error::TimerError => write!(f, "TimerError"),
            Error::Unreachable => write!(f, "Unreachable"),
            Error::JSON(s) => write!(f, "JSON Error: {}", s),
        }
    }
}

impl From<crate::node::Error> for Error {
    fn from(err: crate::node::Error) -> Self {
        Error::Node(err)
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(err: std::net::AddrParseError) -> Self {
        Error::AddrParse(err.description().to_string())
    }
}

impl From<hyper::error::Error> for Error {
    fn from(err: hyper::error::Error) -> Self {
        Error::Hyper(err.description().to_string())
    }
}

impl From<hyper::http::Error> for Error {
    fn from(err: hyper::http::Error) -> Self {
        Error::HyperHTTP(err.description().to_string())
    }
}

impl From<tokio::sync::mpsc::error::UnboundedRecvError> for Error {
    fn from(_: tokio::sync::mpsc::error::UnboundedRecvError) -> Self {
        Error::MPSCRecv
    }
}

impl From<tokio::sync::mpsc::error::UnboundedSendError> for Error {
    fn from(err: tokio::sync::mpsc::error::UnboundedSendError) -> Self {
        Error::MPSCSend(err.description().to_string())
    }
}

impl<T> From<tokio::sync::mpsc::error::UnboundedTrySendError<T>> for Error {
    fn from(_: tokio::sync::mpsc::error::UnboundedTrySendError<T>) -> Self {
        Error::MPSCTrySend
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Error::OneShotRecv
    }
}

impl From<tokio::timer::Error> for Error {
    fn from(_: tokio::timer::Error) -> Self {
        Error::TimerError
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::JSON(format!("{:#?}", err))
    }
}
