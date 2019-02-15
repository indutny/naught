extern crate hyper;

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

impl From<std::net::AddrParseError> for Error {
    fn from(_: std::net::AddrParseError) -> Error {
        Error::Every
    }
}

impl From<hyper::error::Error> for Error {
    fn from(_: hyper::error::Error) -> Error {
        Error::Every
    }
}

impl From<hyper::http::Error> for Error {
    fn from(_: hyper::http::Error) -> Error {
        Error::Every
    }
}
