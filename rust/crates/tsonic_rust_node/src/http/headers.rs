use std::collections::BTreeMap;
use std::rc::Rc;

use crate::buffer::Buffer;
use crate::error::{NodeError, NodeResult};
use crate::net;
use crate::stream::{Readable, Writable};

pub const MAX_HEADER_SIZE: usize = 16 * 1024;

pub fn methods() -> Vec<&'static str> {
    vec![
        "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "TRACE", "CONNECT",
    ]
}

pub fn status_codes() -> BTreeMap<u16, &'static str> {
    [
        (100, "Continue"),
        (101, "Switching Protocols"),
        (102, "Processing"),
        (103, "Early Hints"),
        (200, "OK"),
        (201, "Created"),
        (202, "Accepted"),
        (204, "No Content"),
        (301, "Moved Permanently"),
        (302, "Found"),
        (304, "Not Modified"),
        (400, "Bad Request"),
        (401, "Unauthorized"),
        (403, "Forbidden"),
        (404, "Not Found"),
        (409, "Conflict"),
        (418, "I'm a Teapot"),
        (429, "Too Many Requests"),
        (500, "Internal Server Error"),
        (502, "Bad Gateway"),
        (503, "Service Unavailable"),
        (504, "Gateway Timeout"),
    ]
    .into_iter()
    .collect()
}

pub fn validate_header_name(name: &str) -> NodeResult<()> {
    if name.is_empty()
        || !name.bytes().all(|byte| {
            matches!(
                byte,
                b'!' | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'|'
                    | b'~'
                    | b'0'..=b'9'
                    | b'a'..=b'z'
                    | b'A'..=b'Z'
            )
        })
    {
        return Err(NodeError::new(
            "ERR_INVALID_HTTP_TOKEN",
            "header name contains invalid characters",
        ));
    }
    Ok(())
}

pub fn validate_header_value(name: &str, value: &str) -> NodeResult<()> {
    validate_header_name(name)?;
    if value
        .bytes()
        .any(|byte| matches!(byte, 0..=8 | 10..=31 | 127))
    {
        return Err(NodeError::new(
            "ERR_INVALID_CHAR",
            "header value contains invalid characters",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct HeaderStore {
    names: Vec<String>,
    values: BTreeMap<String, Vec<String>>,
}

impl HeaderStore {
    fn append(&mut self, name: &str, values: impl IntoIterator<Item = String>) -> NodeResult<()> {
        validate_header_name(name)?;
        let key = name.to_ascii_lowercase();
        let values = values.into_iter().collect::<Vec<_>>();
        for value in &values {
            validate_header_value(name, value)?;
        }
        if !self.values.contains_key(&key) {
            self.names.push(key.clone());
        }
        self.values.entry(key).or_default().extend(values);
        Ok(())
    }

    fn set(&mut self, name: &str, values: impl IntoIterator<Item = String>) -> NodeResult<()> {
        validate_header_name(name)?;
        let key = name.to_ascii_lowercase();
        let values = values.into_iter().collect::<Vec<_>>();
        if values.is_empty() {
            return Err(NodeError::new(
                "ERR_HTTP_INVALID_HEADER_VALUE",
                "a response header must contain at least one value",
            ));
        }
        for value in &values {
            validate_header_value(name, value)?;
        }
        if !self.values.contains_key(&key) {
            self.names.push(key.clone());
        }
        self.values.insert(key, values);
        Ok(())
    }

    fn get(&self, name: &str) -> NodeResult<Option<String>> {
        validate_header_name(name)?;
        Ok(self
            .values
            .get(&name.to_ascii_lowercase())
            .and_then(|values| values.first())
            .cloned())
    }

    fn get_all(&self, name: &str) -> NodeResult<Vec<String>> {
        validate_header_name(name)?;
        Ok(self
            .values
            .get(&name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default())
    }

    fn contains(&self, name: &str) -> NodeResult<bool> {
        validate_header_name(name)?;
        Ok(self.values.contains_key(&name.to_ascii_lowercase()))
    }

    fn remove(&mut self, name: &str) -> NodeResult<()> {
        validate_header_name(name)?;
        let key = name.to_ascii_lowercase();
        self.values.remove(&key);
        self.names.retain(|current| current != &key);
        Ok(())
    }

    pub(crate) fn entries(&self) -> Vec<(String, Vec<String>)> {
        self.names
            .iter()
            .filter_map(|name| {
                self.values
                    .get(name)
                    .map(|values| (name.clone(), values.clone()))
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IncomingHttpHeaders {
    store: Rc<HeaderStore>,
}

impl IncomingHttpHeaders {
    pub(crate) fn from_pairs(
        pairs: impl IntoIterator<Item = (String, String)>,
    ) -> NodeResult<Self> {
        let mut store = HeaderStore::default();
        for (name, value) in pairs {
            store.append(&name, [value])?;
        }
        Ok(Self {
            store: Rc::new(store),
        })
    }

    pub fn get(&self, name: &str) -> NodeResult<Option<String>> {
        self.store.get(name)
    }

    pub fn get_all(&self, name: &str) -> NodeResult<tsonic_rust_js::JsArray<String>> {
        self.store
            .get_all(name)
            .map(tsonic_rust_js::JsArray::from_dense)
    }

    pub fn get_values(&self, name: &str) -> NodeResult<Option<tsonic_rust_js::JsArray<String>>> {
        validate_header_name(name)?;
        Ok(self.store.values.get(&name.to_ascii_lowercase())
            .map(|values| tsonic_rust_js::JsArray::from_dense(values.clone())))
    }

    pub fn names(&self) -> tsonic_rust_js::JsArray<String> {
        tsonic_rust_js::JsArray::from_dense(self.store.names.clone())
    }

    pub fn entries(&self) -> Vec<(String, Vec<String>)> {
        self.store.entries()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutgoingHttpHeaders {
    store: HeaderStore,
}

impl OutgoingHttpHeaders {
    fn snapshot(store: &HeaderStore) -> Self {
        Self {
            store: store.clone(),
        }
    }

    pub fn get(&self, name: &str) -> NodeResult<Option<String>> {
        self.store.get(name)
    }

    pub fn get_all(&self, name: &str) -> NodeResult<tsonic_rust_js::JsArray<String>> {
        self.store
            .get_all(name)
            .map(tsonic_rust_js::JsArray::from_dense)
    }

    pub fn names(&self) -> tsonic_rust_js::JsArray<String> {
        tsonic_rust_js::JsArray::from_dense(self.store.names.clone())
    }

    pub fn entries(&self) -> Vec<(String, Vec<String>)> {
        self.store.entries()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InformationEvent {
    pub status_code: u16,
    pub status_message: String,
    pub http_version: String,
    pub http_version_major: u8,
    pub http_version_minor: u8,
    pub headers: IncomingHttpHeaders,
    pub raw_headers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProxyEnv {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub no_proxy: Option<String>,
    pub http_proxy_upper: Option<String>,
    pub https_proxy_upper: Option<String>,
    pub no_proxy_upper: Option<String>,
}
