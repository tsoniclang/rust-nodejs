use std::collections::BTreeMap;

use crate::error::{NodeError, NodeResult};
use crate::http::{IncomingMessage, Response, ServerResponseHandle};
use crate::tls::{SourceServerOptions, TlsServer, TlsSocket};

type RuntimeRequestArguments = (IncomingMessage, ServerResponseHandle);
type RuntimeResponseCallback = tsonic_rust_runtime::Callable<
    IncomingMessage,
    tsonic_rust_runtime::TsonicResult<()>,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestOptions {
    pub url: String,
    pub method: String,
    pub headers: BTreeMap<String, String>,
    pub timeout: Option<u64>,
    pub reject_unauthorized: bool,
}

impl RequestOptions {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: "GET".to_string(),
            headers: BTreeMap::new(),
            timeout: None,
            reject_unauthorized: true,
        }
    }
}

#[derive(Clone)]
pub struct ServerHandle {
    server: TlsServer,
}

#[derive(Clone)]
pub struct ClientRequest {
    options: RequestOptions,
    body: Vec<u8>,
    response_callback: Option<RuntimeResponseCallback>,
    ended: bool,
}

impl ClientRequest {
    fn new(options: RequestOptions, response_callback: Option<RuntimeResponseCallback>) -> Self {
        Self {
            options,
            body: Vec::new(),
            response_callback,
            ended: false,
        }
    }

    pub fn write_buffer(&mut self, buffer: &crate::buffer::Buffer) -> NodeResult<bool> {
        if self.ended {
            return Err(NodeError::new("ERR_STREAM_WRITE_AFTER_END", "write after end"));
        }
        buffer.with_bytes(|bytes| self.body.extend_from_slice(bytes));
        Ok(true)
    }

    pub fn write_string(&mut self, value: &str) -> NodeResult<bool> {
        if self.ended {
            return Err(NodeError::new("ERR_STREAM_WRITE_AFTER_END", "write after end"));
        }
        self.body.extend_from_slice(value.as_bytes());
        Ok(true)
    }

    pub fn end(&mut self) -> NodeResult<()> {
        if self.ended {
            return Ok(());
        }
        self.ended = true;
        let response = request(&self.options, &self.body)?;
        if let Some(callback) = self.response_callback.clone() {
            let message = incoming_response(&self.options.url, response);
            crate::event_loop::enqueue_runtime_task(move || callback.call(message))?;
        }
        Ok(())
    }
}

impl ServerHandle {
    pub fn listen<E>(
        &mut self,
        port: i32,
        host: &str,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.server.listen(source_port(port)?, host)?;
        let callback = adapt_callback(callback);
        crate::event_loop::enqueue_runtime_task(move || callback.call(()))?;
        Ok(self)
    }

    pub fn listen_default_host<E>(
        &mut self,
        port: i32,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen(port, "0.0.0.0", callback)
    }

    pub fn close(&mut self) {
        self.server.close();
    }

    pub fn ref_chain(&mut self) -> &mut Self {
        self.server.ref_chain();
        self
    }

    pub fn unref_chain(&mut self) -> &mut Self {
        self.server.unref_chain();
        self
    }

    pub fn listening(&self) -> bool {
        self.server.listening()
    }
}

pub fn create_server_callable<E>(
    options: SourceServerOptions,
    handler: tsonic_rust_runtime::Callable<RuntimeRequestArguments, Result<(), E>>,
) -> NodeResult<ServerHandle>
where
    E: std::fmt::Display + 'static,
{
    let handler = adapt_callback(handler);
    let connection_callback = tsonic_rust_runtime::Callable::new(
        move |(socket,): (TlsSocket,)| {
            crate::http::accept_runtime_transport(Box::new(socket), handler.clone())
        },
    );
    Ok(ServerHandle {
        server: crate::tls::create_server(options, connection_callback)?,
    })
}

pub fn get(url: &str) -> NodeResult<Response> {
    request(&RequestOptions::get(url), &[])
}

pub fn request_callable<E>(
    url: &str,
    callback: tsonic_rust_runtime::Callable<IncomingMessage, Result<(), E>>,
) -> NodeResult<ClientRequest>
where
    E: std::fmt::Display + 'static,
{
    Ok(ClientRequest::new(
        RequestOptions::get(url),
        Some(adapt_callback(callback)),
    ))
}

pub fn get_callable<E>(
    url: &str,
    callback: tsonic_rust_runtime::Callable<IncomingMessage, Result<(), E>>,
) -> NodeResult<ClientRequest>
where
    E: std::fmt::Display + 'static,
{
    let mut request = request_callable(url, callback)?;
    request.end()?;
    Ok(request)
}

pub fn request(options: &RequestOptions, body: &[u8]) -> NodeResult<Response> {
    if !options.url.starts_with("https://") {
        return Err(NodeError::new(
            "ERR_INVALID_PROTOCOL",
            "https request requires an https:// URL",
        ));
    }
    let mut client = reqwest::blocking::Client::builder().use_rustls_tls();
    if !options.reject_unauthorized {
        client = client.danger_accept_invalid_certs(true);
    }
    if let Some(timeout) = options.timeout {
        client = client.timeout(std::time::Duration::from_millis(timeout));
    }
    let client = client.build().map_err(map_reqwest_error)?;
    let method = options
        .method
        .parse::<reqwest::Method>()
        .map_err(|error| NodeError::new("ERR_INVALID_ARG_VALUE", error.to_string()))?;
    let mut request = client.request(method, &options.url);
    for (name, value) in &options.headers {
        request = request.header(name, value);
    }
    response_to_node(request.body(body.to_vec()).send().map_err(map_reqwest_error)?)
}

pub(crate) fn response_to_node(response: reqwest::blocking::Response) -> NodeResult<Response> {
    let status_code = response.status().as_u16();
    let status_message = response
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string();
    let mut headers = BTreeMap::new();
    for (name, value) in response.headers() {
        let value = value
            .to_str()
            .map_err(|error| NodeError::new("ERR_INVALID_HTTP_TOKEN", error.to_string()))?;
        headers.insert(name.as_str().to_ascii_lowercase(), value.to_string());
    }
    let body = response.bytes().map_err(map_reqwest_error)?.to_vec();
    Ok(Response {
        status_code,
        status_message,
        headers,
        body,
    })
}

fn adapt_callback<TArguments, E>(
    callback: tsonic_rust_runtime::Callable<TArguments, Result<(), E>>,
) -> tsonic_rust_runtime::Callable<TArguments, tsonic_rust_runtime::TsonicResult<()>>
where
    TArguments: 'static,
    E: std::fmt::Display + 'static,
{
    tsonic_rust_runtime::Callable::new(move |arguments| {
        callback
            .call(arguments)
            .map_err(crate::error::callback_runtime_error)
    })
}

fn source_port(value: i32) -> NodeResult<f64> {
    let value = u16::try_from(value)
        .map_err(|_| NodeError::new("ERR_SOCKET_BAD_PORT", "port must be between 0 and 65535"))?;
    Ok(f64::from(value))
}

fn map_reqwest_error(error: reqwest::Error) -> NodeError {
    NodeError::new("ERR_NETWORK", error.to_string())
}

fn incoming_response(url: &str, response: Response) -> IncomingMessage {
    let mut message = IncomingMessage::new("GET", url, response.body);
    message.status_code = Some(response.status_code);
    message.status_message = Some(response.status_message);
    message.headers = response.headers;
    message
}
