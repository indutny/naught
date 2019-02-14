fn main() {
    let mut node = naught::node::Node::new();

    node.listen(3000, "::").expect("Listen to not fail");
}
