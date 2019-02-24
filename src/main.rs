#[macro_use]
extern crate log;

extern crate clap;
extern crate futures;
extern crate serde_json;
extern crate tokio;

use clap::{App, Arg};
use futures::prelude::*;

use naught::config::Config;
use naught::server::Server;

use std::fs::File;
use std::io::prelude::*;

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
                .takes_value(true)
                .default_value("0"),
        )
        .arg(
            Arg::with_name("host")
                .short("h")
                .help("Sets a host to listen on")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::with_name("config")
                .short("c")
                .help("Path to configuration file")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let config = if let Some(config) = matches.value_of("config") {
        let mut file = File::open(config).expect("config file to open");
        let mut s = String::new();

        file.read_to_string(&mut s)
            .expect("config file to be readable");

        serde_json::from_str::<Config>(&s).expect("config to be valid JSON")
    } else {
        panic!("config file not present");
    };

    let port: u16 = matches
        .value_of("port")
        .unwrap()
        .parse()
        .expect("Invalid port value");
    let host = matches.value_of("host").unwrap();

    let server = Server::new(config);
    let listen = server.listen(port, host);

    tokio::run(listen.map_err(|err| {
        error!("Got error {:#?}", err);
        panic!("Done");
    }));
}
