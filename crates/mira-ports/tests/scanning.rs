//! The real socket table, against a socket the test owns.
//!
//! Binding the listener here is what makes this deterministic: it does not matter
//! what else is running on the developer's machine, because the assertion is
//! about a port this process just opened and will close on the way out.

use std::net::TcpListener;

use mira_ports::{PortScanner, Ports};

#[test]
fn a_socket_this_process_is_listening_on_is_found() {
    let socket = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = socket.local_addr().expect("addr").port();

    let listeners = Ports::new().listening().expect("read the socket table");
    let found = listeners
        .iter()
        .find(|listener| listener.port == port)
        .unwrap_or_else(|| {
            panic!(
                "port {port} was not found among {} listeners",
                listeners.len()
            )
        });

    assert_eq!(
        found.pid,
        Some(std::process::id()),
        "the socket is attributed to the process that opened it"
    );
    assert!(found.local_address.contains("127.0.0.1"));
}

#[test]
fn a_closed_socket_stops_being_reported() {
    let port = {
        let socket = TcpListener::bind("127.0.0.1:0").expect("bind");
        socket.local_addr().expect("addr").port()
        // dropped here
    };

    let listeners = Ports::new().listening().expect("read the socket table");

    assert!(
        !listeners
            .iter()
            .any(|listener| listener.port == port && listener.pid == Some(std::process::id())),
        "a service that stopped disappears rather than lingering"
    );
}

#[test]
fn one_port_is_reported_once_per_process() {
    // A server bound on both IPv4 and IPv6 is one service. Reporting it twice
    // would read as two, which is the sort of small lie that erodes trust in the
    // whole list.
    let socket = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = socket.local_addr().expect("addr").port();

    let listeners = Ports::new().listening().expect("read the socket table");
    let mine: Vec<_> = listeners
        .iter()
        .filter(|listener| listener.port == port)
        .collect();

    assert_eq!(mine.len(), 1, "{mine:#?}");
}

#[test]
fn the_scan_comes_back_sorted_by_port() {
    // Sorted so the list does not reshuffle between reads. Only the ordering is
    // asserted, not the contents: the machine's socket table genuinely changes
    // from one moment to the next, and a test that demanded two identical reads
    // would be testing the developer's other applications.
    let _socket = TcpListener::bind("127.0.0.1:0").expect("bind");

    let listeners = Ports::new().listening().expect("read");
    let ports: Vec<u16> = listeners.iter().map(|listener| listener.port).collect();

    assert!(
        ports.windows(2).all(|pair| pair[0] <= pair[1]),
        "sorted by port: {ports:?}"
    );
}
