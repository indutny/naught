use naught::server::Server;

fn main() {
    let server = Server::new();
    server.listen(8000, "::").expect("Listen to not fail");
}
