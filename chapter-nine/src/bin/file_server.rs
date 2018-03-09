extern crate futures;
extern crate hyper;

use hyper::{Method, StatusCode};
use hyper::server::{const_service, service_fn, Http, Request, Response};
use hyper::header::ContentLength;
use std::net::SocketAddr;
use std::thread;
use futures::Future;
use futures::sync::oneshot;
use std::fs::File;
use std::io::{self, copy};

fn main() {
    let addr = "[::1]:3000".parse().expect("Failed to parse address");
    run_echo_server(&addr).expect("Failed to run webserver");
}

fn run_echo_server(addr: &SocketAddr) -> Result<(), hyper::Error> {
    let echo = const_service(service_fn(|req: Request| {
        match (req.method(), req.path()) {
            (&Method::Get, "/") => handle_root(),
            (&Method::Get, file) => handle_get_file(file),
            _ => handle_invalid_method(),
        }
    }));

    let server = Http::new().bind(addr, echo)?;
    server.run()
}

type ResponseFuture = Box<Future<Item = Response, Error = hyper::Error>>;
fn handle_root() -> ResponseFuture {
    send_file_or_404("index.html")
}

fn handle_get_file(file: &str) -> ResponseFuture {
    send_file_or_404(file)
}

fn handle_invalid_method() -> ResponseFuture {
    let response_future = send_file_or_404("invalid_method.html")
        .and_then(|response| Ok(response.with_status(StatusCode::MethodNotAllowed)));
    Box::new(response_future)
}

fn send_file_or_404(path: &str) -> ResponseFuture {
    let not_found_future = try_to_send_file("not_found.html").and_then(|response_result| {
        Ok(response_result.unwrap_or(Response::new().with_status(StatusCode::NotFound)))
    });

    let file_send_future = try_to_send_file(path)
        .and_then(|response_result| response_result.map_err(|error| error.into()))
        .or_else(|_| not_found_future);

    Box::new(file_send_future)
}

type ResponseResultFuture = Box<Future<Item = Result<Response, io::Error>, Error = hyper::Error>>;
fn try_to_send_file(path: &str) -> ResponseResultFuture {
    let path = path_on_disk(path);
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(err) => {
                tx.send(Err(err)).expect("Send error on file not found");
                return;
            }
        };
        let mut buf: Vec<u8> = Vec::new();
        match copy(&mut file, &mut buf) {
            Ok(_) => {
                let res = Response::new()
                    .with_header(ContentLength(buf.len() as u64))
                    .with_body(buf);
                tx.send(Ok(res))
                    .expect("Send error on successful file read");
            }
            Err(err) => {
                tx.send(Err(err)).expect("Send error on error reading file");
            }
        };
    });
    Box::new(rx.map_err(|error| io::Error::new(io::ErrorKind::Other, error).into()))
}

fn path_on_disk(path_to_file: &str) -> String {
    "files/".to_string() + path_to_file
}
