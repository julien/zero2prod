use zero2prod::run;

use std::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8000").expect("failed to bind port 8000");
    // Bubble up the io::Error if we failed to bind the address
    // Otherwise call .await on our server
    run(listener)?.await
}
