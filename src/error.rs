extern crate hyper;
extern crate serde;
extern crate tokio;
extern crate wasmi;

use std::error::Error as StdError;
use std::fmt;

use serde::Serialize;

#[derive(Serialize, Debug)]
pub enum Error {
    AddrParse(String),
    Hyper(String),
    HyperHTTP(String),
    TimerError,
    Wasm(String),
    WasmTrap(String),
    NotFound,
    StoreFailed(String),
    PingFailed,
    BadRequest,
    NonLocalStore(String),
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
            Error::AddrParse(s) => write!(f, "AddrParse: {}", s),
            Error::Hyper(s) => write!(f, "Hyper: {}", s),
            Error::HyperHTTP(s) => write!(f, "Hyper HTTP: {}", s),
            Error::TimerError => write!(f, "TimerError"),
            Error::Wasm(s) => write!(f, "Wasm: {}", s),
            Error::WasmTrap(s) => write!(f, "WasmTrap: {}", s),
            Error::Unreachable => write!(f, "Unreachable"),
            Error::NotFound => write!(f, "Resource not found"),
            Error::StoreFailed(s) => write!(f, "Resource {} store failed", s),
            Error::PingFailed => write!(f, "Remote ping failed"),
            Error::BadRequest => write!(f, "Unsupported request method or uri"),
            Error::NonLocalStore(s) => write!(f, "Cannot store {} locally", s),
            Error::JSON(s) => write!(f, "JSON Error: {}", s),
        }
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

impl From<wasmi::Error> for Error {
    fn from(err: wasmi::Error) -> Self {
        Error::Wasm(err.description().to_string())
    }
}

impl From<wasmi::Trap> for Error {
    fn from(err: wasmi::Trap) -> Self {
        Error::WasmTrap(err.description().to_string())
    }
}
