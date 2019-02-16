use naught::config::Config;
use naught::server::Server;

fn main() {
    env_logger::init();

    let config = Config::new((0u64, 0u64));
    let server = Server::new(config);
    server.listen(8000, "::").expect("Listen to not fail");
}
