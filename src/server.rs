extern crate futures;
extern crate hyper;
extern crate tokio;

extern crate env_logger;

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures::prelude::*;
use tokio::timer::Interval;

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

        let node = Node::new(bind_addr, self.config.clone());
        let node = Arc::new(Mutex::new(node));

        let serve_node = node.clone();
        let server = builder
            .serve(move || RPCService::new(serve_node.clone()))
            .from_err();

        let interval_node = node.clone();
        let interval = Interval::new(Instant::now(), self.config.ping_every.min)
            .from_err::<Error>()
            .for_each(move |_| {
                let ping_node = interval_node.clone();

                interval_node
                    .lock()
                    .expect("lock to acquire")
                    .send_pings()
                    .and_then(|pings| {
                        pings
                            .into_iter()
                            .filter_map(|ping| ping)
                            .for_each(move |ping| {
                                ping_node
                                    .lock()
                                    .expect("lock to acquire")
                                    .recv_ping(&ping)
                                    // Ignore errors
                                    .map(|_| ())
                                    .unwrap_or(());
                            });
                        Ok(())
                    })
            });

        hyper::rt::run(
            server
                .join(interval)
                .map_err(|err: Error| {
                    eprintln!("Got error: {:#?}", err);
                })
                .map(|_| ()),
        );

        Ok(())
    }
}
