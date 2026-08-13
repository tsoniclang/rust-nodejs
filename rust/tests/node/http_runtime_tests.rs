use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::thread;

use tsonic_rust_node::{buffer::Buffer, http, run_event_loop};
use tsonic_rust_runtime::Callable;

#[test]
fn translated_http_server_runs_callbacks_on_the_event_loop_thread() {
    let event_thread = thread::current().id();
    let request_on_event_thread = Rc::new(Cell::new(false));
    let listen_on_event_thread = Rc::new(Cell::new(false));
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));

    let callback_thread = Rc::clone(&request_on_event_thread);
    let callback_server = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(request, response): (http::IncomingMessage, http::ServerResponseHandle)| {
            callback_thread.set(thread::current().id() == event_thread);
            assert_eq!(request.url(), "/asset.bin");
            response.set_status_code(201);
            response
                .set_header(
                    &"content-type".to_string(),
                    &"application/octet-stream".to_string(),
                )
                .unwrap();
            response.end_buffer(Buffer::from_bytes(vec![0, 255, 1]));
            callback_server.borrow().as_ref().unwrap().close();
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());

    let listen_thread = Rc::clone(&listen_on_event_thread);
    server
        .listen_default_host(
            0,
            Callable::new(move |_| listen_thread.set(thread::current().id() == event_thread)),
        )
        .unwrap();
    let port = server.local_port().unwrap();

    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(b"GET /asset.bin HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });

    run_event_loop().unwrap();
    let response = client.join().unwrap();
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    let headers = String::from_utf8(response[..header_end].to_vec()).unwrap();
    assert!(headers.starts_with("HTTP/1.1 201 Created\r\n"));
    assert!(headers.contains("content-type: application/octet-stream\r\n"));
    assert!(headers.contains("Content-Length: 3\r\n"));
    assert_eq!(&response[header_end + 4..], &[0, 255, 1]);
    assert!(request_on_event_thread.get());
    assert!(listen_on_event_thread.get());
}

#[test]
fn translated_http_server_propagates_fallible_callback_errors() {
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));
    let callback_server = Rc::clone(&server_slot);
    let server = http::create_server_fallible_callable(Callable::new(
        move |(_request, _response): (http::IncomingMessage, http::ServerResponseHandle)| {
            callback_server.borrow().as_ref().unwrap().close();
            Err(tsonic_rust_node::NodeError::new("ERR_CALLBACK", "request callback failed").into())
        },
    ))
    .unwrap();
    *server_slot.borrow_mut() = Some(server.clone());
    server
        .listen_default_host_fallible(0, Callable::new(|()| Ok(())))
        .unwrap();
    let port = server.local_port().unwrap();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });

    let error = run_event_loop().unwrap_err();
    assert!(error.to_string().contains("request callback failed"));
    assert!(client.join().unwrap().is_empty());
}

#[test]
fn translated_http_response_supports_text_and_empty_bodies() {
    let text = round_trip_single_response(|response| response.end_string("hello"));
    let text_body = response_body(&text);
    assert_eq!(text_body, b"hello");

    let empty = round_trip_single_response(|response| response.end_empty());
    let empty_body = response_body(&empty);
    assert!(empty_body.is_empty());
}

fn round_trip_single_response(finish: impl Fn(http::ServerResponseHandle) + 'static) -> Vec<u8> {
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));
    let callback_server = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(_request, response): (http::IncomingMessage, http::ServerResponseHandle)| {
            finish(response);
            callback_server.borrow().as_ref().unwrap().close();
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());
    server
        .listen_default_host(0, Callable::new(|_| {}))
        .unwrap();
    let port = server.local_port().unwrap();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });
    run_event_loop().unwrap();
    client.join().unwrap()
}

fn response_body(response: &[u8]) -> &[u8] {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    &response[header_end + 4..]
}
