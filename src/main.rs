#[macro_use]
extern crate log;

extern crate clap;
extern crate futures;
extern crate tokio;

use clap::{App, Arg};
use futures::prelude::*;

use naught::config::Config;
use naught::server::Server;

fn main() {
    env_logger::init();

    let matches = App::with_defaults("naught")
        .version(clap::crate_version!())
        .author(clap::crate_authors!("\n"))
        .about("Naught server")
        .arg(
            Arg::with_name("port")
                .short("p")
                .help("Sets a port to listen on")
                .default_value("0"),
        )
        .arg(
            Arg::with_name("host")
                .short("h")
                .help("Sets a host to listen on")
                .default_value("127.0.0.1"),
        )
        .get_matches();

    let port: u16 = matches
        .value_of("port")
        .unwrap()
        .parse()
        .expect("Invalid port value");
    let host = matches.value_of("host").unwrap();

    let config = Config::new(vec![0], (0u64, 0u64));
    let server = Server::new(config);
    let listen = server.listen(port, host);

    tokio::run(listen.map_err(|err| {
        error!("Got error {:#?}", err);
        panic!("Done");
    }));
}
