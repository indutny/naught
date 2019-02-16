extern crate futures;
extern crate hyper;
extern crate tokio;

use std::net::{IpAddr, SocketAddr};

use futures::prelude::*;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::error::Error;
use crate::node::Node;
use crate::service::*;

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Server {
        Server { config }
    }

    pub fn listen(&self, port: u16, host: &str) -> Result<(), Error> {
        let ip_addr: IpAddr = host.parse().map_err(Error::from)?;
        let bind_addr: SocketAddr = SocketAddr::new(ip_addr, port);

        let builder = hyper::Server::bind(&bind_addr);

        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let server = builder.serve(move || RPCService::new(request_tx.clone()));

        let node = Node::new(server.local_addr(), self.config.clone());
        let rpc = RPCService::run_rpc(request_rx, node);

        hyper::rt::run(
            server
                .from_err()
                .join(rpc)
                .map(|_| ())
                .map_err(move |err: Error| {
                    eprintln!("Got error: {:#?}", err);
                }),
        );

        Ok(())
    }
}
