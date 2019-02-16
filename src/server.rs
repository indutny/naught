extern crate futures;
extern crate hyper;
extern crate tokio;

extern crate env_logger;

use std::net::{IpAddr, SocketAddr};
use std::time::Instant;

use futures::future;
use futures::prelude::*;
use tokio::sync::{mpsc, oneshot};
use tokio::timer::Interval;

use crate::config::Config;
use crate::error::Error;
use crate::message::rpc::{RequestMessage, RequestPacket, ResponseMessage};
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
        let poll_tx = request_tx.clone();
        let server = builder.serve(move || RPCService::new(request_tx.clone()));

        let node = Node::new(server.local_addr(), self.config.clone());

        let interval = Interval::new(Instant::now(), self.config.ping_every)
            .from_err::<Error>()
            .for_each(move |_| {
                let (response_tx, response_rx) = oneshot::channel();

                let packet = RequestPacket {
                    response_tx,
                    message: RequestMessage::SendPing,
                };

                poll_tx
                    .clone()
                    .send(packet)
                    .from_err::<Error>()
                    .and_then(move |_| response_rx.from_err())
                    .and_then(|res_packet| {
                        let peers = match res_packet.message {
                            ResponseMessage::SendPing(msg) => msg.peers,
                            _ => {
                                return future::err(Error::Unreachable);
                            }
                        };

                        // TODO(indutny): ping the peers
                        future::ok(())
                    })
            });

        let rpc = RPCService::run_rpc(request_rx, node);

        hyper::rt::run(
            server
                .from_err()
                .join(rpc)
                .join(interval)
                .map(|_| ())
                .map_err(move |err: Error| {
                    eprintln!("Got error: {:#?}", err);
                }),
        );

        Ok(())
    }
}
