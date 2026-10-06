use crate::error::{NodeError, NodeResult};
use crate::http::{IncomingMessage, Response, ServerResponse};
use crate::tls::{SourceServerOptions, TlsServer, TlsSocket};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

type RuntimeRequestArguments<E> = (IncomingMessage<E>, ServerResponse<E>);
type RuntimeResponseCallback<E> =
    tsonic_rust_runtime::Callable<(IncomingMessage<E>,), Result<(), E>>;

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

pub struct ServerHandle<E: 'static = NodeError> {
    server: TlsServer<E>,
}
impl<E: 'static> Clone for ServerHandle<E> {
    fn clone(&self) -> Self {
        Self {
            server: self.server.clone(),
        }
    }
}
struct ClientRequestState<E: 'static> {
    options: RequestOptions,
    body: Vec<u8>,
    response_callback: Option<RuntimeResponseCallback<E>>,
    background: crate::background::BackgroundHandle<E>,
    ended: bool,
}
pub struct ClientRequest<E: 'static = NodeError> {
    state: Rc<RefCell<ClientRequestState<E>>>,
}
impl<E: 'static> Clone for ClientRequest<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}
impl<E: From<NodeError> + 'static> ClientRequest<E> {
    fn new(
        background: &crate::background::BackgroundTasks<E>,
        options: RequestOptions,
        response_callback: Option<RuntimeResponseCallback<E>>,
    ) -> Self {
        Self {
            state: Rc::new(RefCell::new(ClientRequestState {
                options,
                body: Vec::new(),
                response_callback,
                background: background.handle(),
                ended: false,
            })),
        }
    }
    pub fn write_buffer(&self, buffer: &crate::buffer::Buffer) -> NodeResult<bool> {
        let mut state = self.state.borrow_mut();
        if state.ended {
            return Err(write_after_end());
        }
        buffer.with_bytes(|bytes| state.body.extend_from_slice(bytes));
        Ok(true)
    }
    pub fn write_string(&self, value: &str) -> NodeResult<bool> {
        let mut state = self.state.borrow_mut();
        if state.ended {
            return Err(write_after_end());
        }
        state.body.extend_from_slice(value.as_bytes());
        Ok(true)
    }
    pub fn end(&self) -> NodeResult<()> {
        let (options, body, callback, background) = {
            let mut state = self.state.borrow_mut();
            if state.ended {
                return Ok(());
            }
            state.ended = true;
            (
                state.options.clone(),
                std::mem::take(&mut state.body),
                state.response_callback.take(),
                state.background.clone(),
            )
        };
        let response_url = options.url.clone();
        background.spawn(
            move || request(&options, &body),
            move |response| {
                let response = response.map_err(E::from)?;
                if let Some(callback) = callback {
                    callback
                        .call((incoming_response(&response_url, response).map_err(E::from)?,))?;
                }
                Ok(())
            },
        )
    }
}
impl<E: From<NodeError> + 'static> ServerHandle<E> {
    pub fn listen(
        &self,
        tasks: &crate::runtime_tasks::RuntimeTasks<E>,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        host: &str,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.server.listen(port, host)?;
        tasks.enqueue(move || callback.call(()))?;
        Ok(self.clone())
    }
    pub fn listen_default_host(
        &self,
        tasks: &crate::runtime_tasks::RuntimeTasks<E>,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.listen(tasks, port, "0.0.0.0", callback)
    }
    pub fn close(&self) {
        self.server.close();
    }
    pub fn ref_chain(&self) -> Self {
        self.server.ref_chain();
        self.clone()
    }
    pub fn unref_chain(&self) -> Self {
        self.server.unref_chain();
        self.clone()
    }
    pub fn listening(&self) -> bool {
        self.server.listening()
    }
}
pub fn create_server_callable<E: From<NodeError> + 'static>(
    http: &crate::http::HttpServers<E>,
    roots: &crate::tls::TlsServers<E>,
    background: &crate::background::BackgroundTasks<E>,
    options: SourceServerOptions,
    handler: tsonic_rust_runtime::Callable<RuntimeRequestArguments<E>, Result<(), E>>,
) -> NodeResult<ServerHandle<E>> {
    let http = http.handle();
    let connection_callback = tsonic_rust_runtime::Callable::new(move |(socket,): (TlsSocket,)| {
        crate::http::accept_runtime_transport(&http, Box::new(socket), handler.clone())
    });
    Ok(ServerHandle {
        server: crate::tls::create_server(roots, background, options, connection_callback)?,
    })
}
pub fn get(url: &str) -> NodeResult<Response> {
    request(&RequestOptions::get(url), &[])
}
pub fn request_callable<E: From<NodeError> + 'static>(
    background: &crate::background::BackgroundTasks<E>,
    url: &str,
    callback: RuntimeResponseCallback<E>,
) -> NodeResult<ClientRequest<E>> {
    Ok(ClientRequest::new(
        background,
        RequestOptions::get(url),
        Some(callback),
    ))
}
pub fn get_callable<E: From<NodeError> + 'static>(
    background: &crate::background::BackgroundTasks<E>,
    url: &str,
    callback: RuntimeResponseCallback<E>,
) -> NodeResult<ClientRequest<E>> {
    let request = request_callable(background, url, callback)?;
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
    response_to_node(
        request
            .body(body.to_vec())
            .send()
            .map_err(map_reqwest_error)?,
    )
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

fn write_after_end() -> NodeError {
    NodeError::new("ERR_STREAM_WRITE_AFTER_END", "write after end")
}
fn map_reqwest_error(error: reqwest::Error) -> NodeError {
    NodeError::new("ERR_NETWORK", error.to_string())
}
fn incoming_response<E: From<NodeError> + 'static>(
    url: &str,
    response: Response,
) -> NodeResult<IncomingMessage<E>> {
    IncomingMessage::from_client_response(
        url.to_string(),
        response.status_code,
        response.status_message,
        "1.1".to_string(),
        response.headers.into_iter().collect(),
        response.body,
    )
}
