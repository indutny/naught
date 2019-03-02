extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate serde_json;

use futures::future;
use futures::prelude::*;
use hyper::client::HttpConnector;
use hyper::{header, Body, Client as HTTPClient, Method, Request, Response};
use hyper_tls::HttpsConnector;

use crate::config::Config;
use crate::data::Data;
use crate::error::Error;
use crate::message::{common, response};

type FuturePing = Box<Future<Item = Option<common::Ping>, Error = Error> + Send>;
type FutureFetch = Box<Future<Item = response::Fetch, Error = Error> + Send>;
type FutureEmpty = Box<Future<Item = (), Error = Error> + Send>;

const CONNECTOR_THREADS: usize = 4;

pub struct Client {
    client: HTTPClient<HttpsConnector<HttpConnector>>,
    sender: String,
    auth: String,
}

impl Client {
    pub fn new(config: &Config, sender: &str) -> Self {
        let connector = HttpsConnector::new(CONNECTOR_THREADS).expect("Connector to instantiate");

        let client = HTTPClient::builder().build::<_, Body>(connector);

        Client {
            client,
            sender: sender.to_string(),
            auth: config.get_auth(),
        }
    }

    pub fn ping(&self, peer_uri: &str, json_ping: &str) -> FuturePing {
        let uri = format!("{}/_ping", peer_uri);

        let request = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::AUTHORIZATION, self.auth.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-naught-sender", self.sender.clone())
            .body(Body::from(json_ping.to_string()));

        let request = match request {
            Ok(request) => request,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        // TODO(indutny): timeout
        let f = self
            .client
            .request(request)
            .from_err::<Error>()
            .and_then(|response| {
                let is_success = if response.status().is_success() {
                    future::ok(())
                } else {
                    future::err(Error::PingFailed)
                };
                is_success.and_then(|_| response.into_body().concat2().from_err())
            })
            .and_then(|chunk| serde_json::from_slice::<common::Ping>(&chunk).map_err(Error::from))
            .and_then(|ping| future::ok(Some(ping)))
            .from_err::<Error>();

        Box::new(f)
    }

    pub fn fetch(&self, peer_uri: &str, container: &str, uri: &str) -> FutureFetch {
        let uri = format!("{}/{}", peer_uri, uri);

        trace!(
            "fetch remote container: {} peer: {} uri: {}",
            container,
            peer_uri,
            uri
        );
        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .header(header::HOST, container.to_string())
            .header("x-naught-sender", self.sender.clone())
            .header("x-naught-redirect", "false")
            .body(Body::empty());

        let request = match request {
            Ok(request) => request,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        let responder = peer_uri.to_string();

        // TODO(indutny): timeout
        let f = self
            .client
            .request(request)
            .from_err::<Error>()
            .and_then(|response| {
                if response.status().is_success() {
                    Ok(response)
                } else {
                    Err(Error::NotFound)
                }
            })
            .map(move |response| {
                let (parts, body) = response.into_parts();

                let mime = parts
                    .headers
                    .get(hyper::header::CONTENT_TYPE)
                    .map(|val| val.to_str().unwrap_or("unknown"))
                    .unwrap_or("unknown")
                    .to_string();
                response::Fetch {
                    peer: responder,
                    mime,
                    body,
                }
            });
        Box::new(f)
    }

    pub fn store(&self, peer_uri: &str, container: &str, data: &Data) -> FutureEmpty {
        trace!("store remote container: {} peer: {}", container, peer_uri);

        let peek = Request::builder()
            .method(Method::HEAD)
            .uri(format!("{}/", peer_uri))
            .header(header::HOST, container.to_string())
            .header("x-naught-sender", self.sender.clone())
            .header("x-naught-redirect", "false")
            .body(Body::empty());

        // TODO(indutny): lazy body?
        let store = Request::builder()
            .method(Method::PUT)
            .uri(format!("{}/_container", peer_uri))
            .header(header::AUTHORIZATION, self.auth.clone())
            .header("x-naught-sender", self.sender.to_string())
            .header("x-naught-redirect", "false")
            .body(Body::from(Vec::from(data)));

        let peek = match peek {
            Ok(peek) => peek,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        let store = match store {
            Ok(store) => store,
            Err(err) => {
                return Box::new(future::err(Error::from(err)));
            }
        };

        let peek = self
            .client
            .request(peek)
            .from_err::<Error>()
            .and_then(|response| {
                if response.status().is_success() {
                    Ok(())
                } else {
                    Err(Error::NotFound)
                }
            });

        // TODO(indutny): excessive cloning?
        let debug_uri = format!("{}/{}", peer_uri, container);

        let on_store_response = move |response: Response<Body>| {
            if response.status().is_success() {
                Ok(())
            } else {
                Err(Error::StoreFailed(debug_uri))
            }
        };

        let store = self
            .client
            .request(store)
            .from_err::<Error>()
            .and_then(on_store_response);

        let peek_or_store = peek.or_else(move |_| store);

        // TODO(indutny): timeout
        // TODO(indutny): retry?
        Box::new(peek_or_store)
    }
}
