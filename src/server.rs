extern crate futures;
extern crate hyper;
extern crate tokio;

extern crate env_logger;

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures::future;
use futures::prelude::*;
use hyper::service::make_service_fn;
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

    pub fn listen(&self, port: u16, host: &str) -> Box<Future<Item = (), Error = Error> + Send> {
        let ip_addr: IpAddr = match host.parse().map_err(Error::from) {
            Ok(addr) => addr,
            Err(err) => {
                return Box::new(future::err(err));
            }
        };
        let bind_addr: SocketAddr = SocketAddr::new(ip_addr, port);

        let builder = hyper::Server::bind(&bind_addr);

        let node = Node::new(bind_addr, self.config.clone());
        let node = Arc::new(Mutex::new(node));

        let serve_node = node.clone();
        let server = builder.serve(make_service_fn(move |stream| {
            RPCService::new(stream, serve_node.clone())
        }));

        node.lock()
            .expect("lock to acquire")
            .set_local_addr(server.local_addr());

        let ping_node = node.clone();
        let ping = Interval::new(Instant::now(), self.config.ping_every.min)
            .from_err::<Error>()
            .for_each(move |_| {
                let node = ping_node.clone();

                ping_node
                    .lock()
                    .expect("lock to acquire")
                    .send_pings()
                    .and_then(|pings| {
                        pings
                            .into_iter()
                            .filter_map(|ping| ping)
                            .for_each(move |ping| {
                                node.lock()
                                    .expect("lock to acquire")
                                    .recv_ping(&ping)
                                    // Ignore errors
                                    .map(|_| ())
                                    .unwrap_or(());
                            });
                        Ok(())
                    })
            });

        let rebalance_node = node.clone();
        let rebalance = Interval::new(Instant::now(), self.config.rebalance_every)
            .from_err::<Error>()
            .for_each(move |_| {
                let remove_keys_node = rebalance_node.clone();

                rebalance_node
                    .lock()
                    .expect("lock to acquire")
                    .rebalance()
                    .map(move |obsolete_keys| {
                        remove_keys_node
                            .lock()
                            .expect("lock to acquire")
                            .remove(obsolete_keys);
                    })
            });

        trace!("Listening on {:?}", server.local_addr());

        Box::new(server.from_err().join(ping).join(rebalance).map(|_| ()))
    }
}
