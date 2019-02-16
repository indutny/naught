extern crate clap;

use clap::{App, Arg};

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
                .default_value("8000"),
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

    let config = Config::new((0u64, 0u64));
    let server = Server::new(config);
    server.listen(port, host).expect("Listen to not fail");
}
