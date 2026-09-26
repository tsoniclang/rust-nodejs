use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::thread;

use tsonic_rust_node::{buffer::Buffer, http, run_event_loop};
use tsonic_rust_runtime::{Callable, TsonicError};

#[test]
fn translated_http_server_runs_callbacks_on_the_event_loop_thread() {
    let event_thread = thread::current().id();
    let request_on_event_thread = Rc::new(Cell::new(false));
    let listen_on_event_thread = Rc::new(Cell::new(false));
    let listening_events = Rc::new(Cell::new(0));
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));

    let callback_thread = Rc::clone(&request_on_event_thread);
    let callback_server = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(request, response): (http::IncomingMessage, http::ServerResponse)| {
            callback_thread.set(thread::current().id() == event_thread);
            assert_eq!(request.url(), Some("/asset.bin".to_string()));
            response.set_status_code(201).unwrap();
            response
                .set_header("content-type", "application/octet-stream")
                .unwrap();
            response
                .end_buffer(&Buffer::from_bytes(vec![0, 255, 1]))
                .unwrap();
            callback_server.borrow().as_ref().unwrap().close().unwrap();
            Ok::<(), TsonicError>(())
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());

    let removed_events = Rc::clone(&listening_events);
    let removed_listener = Callable::new(move |()| {
        removed_events.set(removed_events.get() + 1);
        Ok::<(), TsonicError>(())
    });
    server.on_listening("listening", &removed_listener).unwrap();
    server
        .off_listening("listening", &removed_listener)
        .unwrap();
    let fired_events = Rc::clone(&listening_events);
    let once_listener = Callable::new(move |()| {
        fired_events.set(fired_events.get() + 1);
        Ok::<(), TsonicError>(())
    });
    server.once_listening("listening", &once_listener).unwrap();

    let listen_thread = Rc::clone(&listen_on_event_thread);
    server
        .listen_default_host(
            0,
            Callable::new(move |_| {
                listen_thread.set(thread::current().id() == event_thread);
                Ok::<(), TsonicError>(())
            }),
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
    assert!(headers
        .to_ascii_lowercase()
        .contains("content-length: 3\r\n"));
    assert_eq!(&response[header_end + 4..], &[0, 255, 1]);
    assert!(request_on_event_thread.get());
    assert!(listen_on_event_thread.get());
    assert_eq!(listening_events.get(), 1);
}

#[test]
fn translated_http_server_propagates_fallible_callback_errors() {
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));
    let callback_server = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(_request, _response): (http::IncomingMessage, http::ServerResponse)| {
            callback_server.borrow().as_ref().unwrap().close().unwrap();
            Err(std::io::Error::other("request callback failed"))
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());
    server
        .listen_default_host(0, Callable::new(|()| Ok::<(), TsonicError>(())))
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
    assert!(error.to_string().contains("ERR_TSONIC_CALLBACK"));
    assert!(error.to_string().contains("request callback failed"));
    assert!(client.join().unwrap().is_empty());
}

#[test]
fn translated_http_response_supports_text_and_empty_bodies() {
    let text = round_trip_single_response(|response| {
        response.end_string("hello").unwrap();
    });
    let text_body = response_body(&text);
    assert_eq!(text_body, b"hello");

    let empty = round_trip_single_response(|response| {
        response.end_empty().unwrap();
    });
    let empty_body = response_body(&empty);
    assert!(empty_body.is_empty());
}

#[test]
fn chunked_request_streams_fragmented_binary_body_before_response() {
    let received = Rc::new(RefCell::new(Vec::new()));
    let received_in_handler = Rc::clone(&received);
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));
    let server_in_handler = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(request, response): (http::IncomingMessage, http::ServerResponse)| {
            assert_eq!(request.url(), Some("/binary".to_string()));
            let body = Rc::clone(&received_in_handler);
            request.on_data(
                "data",
                &Callable::new(move |(chunk,): (Buffer,)| {
                    body.borrow_mut().extend_from_slice(&chunk.as_bytes());
                    Ok::<(), TsonicError>(())
                }),
            )?;
            let body = Rc::clone(&received_in_handler);
            let closing_server = Rc::clone(&server_in_handler);
            request.on_end(
                "end",
                &Callable::new(move |()| {
                    response.end_buffer(&Buffer::from_bytes(body.borrow().clone()))?;
                    closing_server.borrow().as_ref().unwrap().close()?;
                    Ok::<(), TsonicError>(())
                }),
            )?;
            Ok::<(), TsonicError>(())
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());
    server
        .listen_default_host(0, Callable::new(|()| Ok::<(), TsonicError>(())))
        .unwrap();
    let port = server.local_port().unwrap();

    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        stream
            .write_all(b"POST /binary HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\n\x00\xff")
            .unwrap();
        thread::sleep(std::time::Duration::from_millis(10));
        stream
            .write_all(b"\x01\x02\r\n0\r\nX-Meta: value\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });

    run_event_loop().unwrap();
    let response = client.join().unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert_eq!(response_body(&response), &[0, 255, 1, 2]);
    assert_eq!(*received.borrow(), vec![0, 255, 1, 2]);
}

#[test]
fn request_streams_beyond_the_previous_transport_size_limit() {
    const BODY_LENGTH: usize = 64 * 1024 * 1024 + 1;
    let received = Rc::new(Cell::new(0usize));
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));
    let callback_received = Rc::clone(&received);
    let callback_server = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(request, response): (http::IncomingMessage, http::ServerResponse)| {
            let chunk_received = Rc::clone(&callback_received);
            request.on_data(
                "data",
                &Callable::new(move |(chunk,): (Buffer,)| {
                    chunk_received.set(chunk_received.get() + chunk.len());
                    Ok::<(), TsonicError>(())
                }),
            )?;
            let completed_received = Rc::clone(&callback_received);
            let closing_server = Rc::clone(&callback_server);
            request.on_end(
                "end",
                &Callable::new(move |()| {
                    response.end_string(&completed_received.get().to_string())?;
                    closing_server.borrow().as_ref().unwrap().close()?;
                    Ok::<(), TsonicError>(())
                }),
            )?;
            Ok::<(), TsonicError>(())
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());
    server
        .listen_default_host(0, Callable::new(|()| Ok::<(), TsonicError>(())))
        .unwrap();
    let port = server.local_port().unwrap();

    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .unwrap();
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(30)))
            .unwrap();
        stream
            .write_all(
                format!(
                    "POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: {BODY_LENGTH}\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .unwrap();
        let chunk = [b'x'; 16 * 1024];
        let mut remaining = BODY_LENGTH;
        while remaining > 0 {
            let length = remaining.min(chunk.len());
            stream.write_all(&chunk[..length]).unwrap();
            remaining -= length;
        }
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });

    run_event_loop().unwrap();
    let response = client.join().unwrap();
    assert_eq!(received.get(), BODY_LENGTH);
    assert_eq!(response_body(&response), BODY_LENGTH.to_string().as_bytes());
}

#[test]
fn pipelined_requests_keep_one_connection_until_the_final_response() {
    let requests = Rc::new(Cell::new(0));
    let requests_in_handler = Rc::clone(&requests);
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));
    let server_in_handler = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(request, response): (http::IncomingMessage, http::ServerResponse)| {
            let sequence = requests_in_handler.get() + 1;
            requests_in_handler.set(sequence);
            if sequence == 1 {
                assert_eq!(request.url(), Some("/first".to_string()));
                response.end_string("first")?;
            } else {
                assert_eq!(sequence, 2);
                assert_eq!(request.url(), Some("/second".to_string()));
                response.end_string("second")?;
                server_in_handler.borrow().as_ref().unwrap().close()?;
            }
            Ok::<(), TsonicError>(())
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());
    server
        .listen_default_host(0, Callable::new(|()| Ok::<(), TsonicError>(())))
        .unwrap();
    let port = server.local_port().unwrap();

    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        stream
            .write_all(b"GET /first HTTP/1.1\r\nHost: localhost\r\n\r\nGET /second HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });

    run_event_loop().unwrap();
    let response = String::from_utf8(client.join().unwrap()).unwrap();
    assert_eq!(requests.get(), 2);
    assert_eq!(response.matches("HTTP/1.1 200 OK\r\n").count(), 2);
    assert!(response.contains("first"));
    assert!(response.contains("second"));
}

#[test]
fn response_pressure_tracks_native_queue_and_drains_once() {
    let drain_count = Rc::new(Cell::new(0));
    let drain_in_handler = Rc::clone(&drain_count);
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));
    let server_in_handler = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(_request, response): (http::IncomingMessage, http::ServerResponse)| {
            let drains = Rc::clone(&drain_in_handler);
            response.on_drain(
                "drain",
                &Callable::new(move |()| {
                    drains.set(drains.get() + 1);
                    Ok::<(), TsonicError>(())
                }),
            )?;
            response.set_header("content-length", "70000")?;
            let accepted = response.write_buffer(&Buffer::from_bytes(vec![b'x'; 70_000]))?;
            assert!(!accepted);
            assert!(response.writable_need_drain());
            response.end_empty()?;
            server_in_handler.borrow().as_ref().unwrap().close()?;
            Ok::<(), TsonicError>(())
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());
    server
        .listen_default_host(0, Callable::new(|()| Ok::<(), TsonicError>(())))
        .unwrap();
    let port = server.local_port().unwrap();

    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        stream
            .write_all(b"GET /large HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    });

    run_event_loop().unwrap();
    let response = client.join().unwrap();
    assert_eq!(response_body(&response), vec![b'x'; 70_000]);
    assert_eq!(drain_count.get(), 1);
}

#[test]
fn explicit_response_materialization_preserves_native_bytes() {
    let response = http::ServerResponse::new();
    response.set_status_code(201).unwrap();
    response
        .write_buffer(&Buffer::from_bytes(vec![0, 255]))
        .unwrap();
    response.end_empty().unwrap();
    let snapshot = response.to_response();
    assert_eq!(snapshot.status_code, 201);
    assert_eq!(snapshot.body, vec![0, 255]);
}

fn round_trip_single_response(finish: impl Fn(http::ServerResponse) + 'static) -> Vec<u8> {
    let server_slot = Rc::new(RefCell::new(None::<http::ServerHandle>));
    let callback_server = Rc::clone(&server_slot);
    let server = http::create_server_callable(Callable::new(
        move |(_request, response): (http::IncomingMessage, http::ServerResponse)| {
            finish(response);
            callback_server.borrow().as_ref().unwrap().close().unwrap();
            Ok::<(), TsonicError>(())
        },
    ));
    *server_slot.borrow_mut() = Some(server.clone());
    server
        .listen_default_host(0, Callable::new(|_| Ok::<(), TsonicError>(())))
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
