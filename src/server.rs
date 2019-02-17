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
use crate::message::common;
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
                let (get_res_tx, get_res_rx) = oneshot::channel();
                let (send_res_tx, send_res_rx) = oneshot::channel();

                let packet = RequestPacket {
                    response_tx: get_res_tx,
                    message: RequestMessage::GetPingURIs,
                };

                let ping_tx = poll_tx.clone();

                poll_tx
                        .clone()
                        .send(packet)
                        .from_err::<Error>()
                        .and_then(move |_| get_res_rx.from_err())
                        .and_then(
                            |res_packet| -> Box<
                                Future<Item = Vec<Option<common::Ping>>, Error = Error> + Send,
                            > {
                                trace!("got interval message {:?}", res_packet.message);

                                let msg = match res_packet.message {
                                    ResponseMessage::GetPingURIs(msg) => msg,
                                    _ => {
                                        return Box::new(future::err(Error::Unreachable));
                                    }
                                };

                                Node::send_pings(msg.ping, msg.peers)
                            },
                        )
                        .and_then(move |pings| {
                            let pings = pings.into_iter().filter_map(|ping| ping).collect();

                            ping_tx
                                .send(RequestPacket {
                                    response_tx: send_res_tx,
                                    message: RequestMessage::RecvPingList(pings),
                                })
                                .from_err()
                        })
                        .and_then(move |_| send_res_rx.from_err())
                        .map(|_| ())
            });

        let rpc = RPCService::run_rpc(request_rx, node);

        hyper::rt::run(
            server
                .from_err()
                .join(rpc)
                .join(interval)
                .map_err(|err: Error| {
                    eprintln!("Got error: {:#?}", err);
                })
                .map(|_| ()),
        );

        Ok(())
    }
}
