extern crate futures;
extern crate hyper;
extern crate tokio;
extern crate tokio_lock;

extern crate env_logger;

use std::net::{IpAddr, SocketAddr};
use std::time::Instant;

use futures::future;
use futures::prelude::*;
use tokio::timer::Interval;
use tokio_lock::Lock;

use crate::config::Config;
use crate::error::Error;
use crate::message::common;
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

        let node = Node::new(bind_addr.clone(), self.config.clone());
        let mut lock = Lock::new();

        let manage_lock = lock.manage(node).from_err::<Error>();

        let http_lock = lock.clone();
        let server = builder
            .serve(move || RPCService::new(http_lock.clone()))
            .from_err();

        let interval_lock = lock.clone();
        let interval = Interval::new(Instant::now(), self.config.ping_every)
            .from_err::<Error>()
            .for_each(move |_| {
                let mut send_lock = interval_lock.clone();
                let mut receive_lock = interval_lock.clone();

                send_lock
                    .get_mut(|node| node.send_pings())
                    .and_then(move |pings| {
                        let pings: Vec<common::Ping> =
                            pings.into_iter().filter_map(|ping| ping).collect();

                        receive_lock.get_mut(move |node| {
                            // TODO(indutny): excessive cloning
                            for ping in pings.clone() {
                                node.recv_ping(ping).map(|_| ()).unwrap_or(());
                            }
                            future::ok(())
                        })
                    })
            });

        hyper::rt::run(
            server
                .join(manage_lock)
                .join(interval)
                .map_err(|err: Error| {
                    eprintln!("Got error: {:#?}", err);
                })
                .map(|_| ()),
        );

        Ok(())
    }
}
