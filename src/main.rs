use naught::node::Node;
use naught::server::Server;

fn main() {
    let node = Node::new();

    let mut server = Server::new();
    server.listen(node, 8000, "::").expect("Listen to not fail");
}
