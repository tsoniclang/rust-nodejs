use crate::error::{NodeError, NodeResult};
use crate::punycode;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    href: String,
    protocol: String,
    username: String,
    password: String,
    host: String,
    hostname: String,
    port: String,
    pathname: String,
    search: String,
    hash: String,
}

impl Url {
    pub fn parse(input: &str, base: Option<&str>) -> NodeResult<Self> {
        let input = if input.contains("://") {
            input.to_string()
        } else if let Some(base) = base {
            join_base(base, input)
        } else {
            return Err(NodeError::new(
                "ERR_INVALID_URL",
                "relative URL without base",
            ));
        };
        parse_absolute_url(&input)
    }

    pub fn href(&self) -> String {
        self.href.clone()
    }

    pub fn protocol(&self) -> String {
        self.protocol.clone()
    }

    pub fn username(&self) -> String {
        self.username.clone()
    }

    pub fn password(&self) -> String {
        self.password.clone()
    }

    pub fn host(&self) -> String {
        self.host.clone()
    }

    pub fn hostname(&self) -> String {
        self.hostname.clone()
    }

    pub fn port(&self) -> String {
        self.port.clone()
    }

    pub fn pathname(&self) -> String {
        self.pathname.clone()
    }

    pub fn search(&self) -> String {
        self.search.clone()
    }

    pub fn hash(&self) -> String {
        self.hash.clone()
    }

    pub fn origin(&self) -> String {
        if self.protocol == "file:" {
            "null".to_string()
        } else {
            format!("{}//{}", self.protocol, self.host)
        }
    }

    pub fn search_params(&self) -> NodeResult<UrlSearchParams> {
        UrlSearchParams::new(Some(self.search.strip_prefix('?').unwrap_or(&self.search)))
    }

    pub fn to_json(&self) -> String {
        self.href()
    }

    pub fn set_hash(&mut self, value: &str) {
        self.hash = if value.is_empty() || value.starts_with('#') {
            value.to_string()
        } else {
            format!("#{value}")
        };
        self.rebuild_href();
    }

    pub fn set_search(&mut self, value: &str) {
        self.search = if value.is_empty() || value.starts_with('?') {
            value.to_string()
        } else {
            format!("?{value}")
        };
        self.rebuild_href();
    }

    pub fn set_pathname(&mut self, value: &str) {
        self.pathname = if value.starts_with('/') {
            value.to_string()
        } else {
            format!("/{value}")
        };
        self.rebuild_href();
    }

    fn rebuild_href(&mut self) {
        self.href = format!(
            "{}//{}{}{}{}",
            self.protocol, self.host, self.pathname, self.search, self.hash
        );
    }
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.href)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyUrlObject {
    pub href: Option<String>,
    pub protocol: Option<String>,
    pub slashes: Option<bool>,
    pub auth: Option<String>,
    pub host: Option<String>,
    pub port: Option<String>,
    pub hostname: Option<String>,
    pub hash: Option<String>,
    pub search: Option<String>,
    pub query: Option<String>,
    pub pathname: Option<String>,
    pub path: Option<String>,
}

impl LegacyUrlObject {
    pub fn href(&self) -> Option<String> {
        self.href.clone()
    }

    pub fn required_href(&self) -> String {
        self.href
            .clone()
            .expect("Url values must carry their required href")
    }

    pub fn set_required_href(&mut self, value: String) {
        self.href = Some(value);
    }

    /// Returns the protocol including the trailing colon (for example
    /// `https:`).
    pub fn protocol(&self) -> Option<String> {
        self.protocol.clone()
    }

    /// Returns the authentication component without the trailing `@`.
    pub fn auth(&self) -> Option<String> {
        self.auth.clone()
    }

    /// Returns the host including the port when present (for example
    /// `example.com:8443`).
    pub fn host(&self) -> Option<String> {
        self.host.clone()
    }

    /// Returns the hostname without the port.
    pub fn hostname(&self) -> Option<String> {
        self.hostname.clone()
    }

    /// Returns the port as a decimal string.
    pub fn port(&self) -> Option<String> {
        self.port.clone()
    }

    /// Returns the pathname (for example `/a/b`).
    pub fn pathname(&self) -> Option<String> {
        self.pathname.clone()
    }

    /// Returns the search string including the leading `?`.
    pub fn search(&self) -> Option<String> {
        self.search.clone()
    }

    /// Returns the pathname and search string.
    pub fn path(&self) -> Option<String> {
        self.path.clone()
    }

    /// Returns whether the URL contains a host-introducing `//` sequence.
    pub fn slashes(&self) -> Option<bool> {
        self.slashes
    }

    /// Returns the query string without the leading `?`.
    pub fn query(&self) -> Option<String> {
        self.query.clone()
    }

    /// Returns the fragment including the leading `#`.
    pub fn hash(&self) -> Option<String> {
        self.hash.clone()
    }
}

pub type UrlObject = LegacyUrlObject;
pub type UrlWithStringQuery = LegacyUrlObject;
pub type UrlWithParsedQuery = LegacyUrlObject;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UrlFormatOptions {
    pub auth: Option<bool>,
    pub fragment: Option<bool>,
    pub search: Option<bool>,
    pub unicode: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileUrlToPathOptions {
    pub windows: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PathToFileUrlOptions {
    pub windows: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpOptions {
    pub protocol: String,
    pub hostname: String,
    pub port: Option<u16>,
    pub path: String,
    pub auth: String,
}

pub fn can_parse(input: &str, base: Option<&str>) -> bool {
    Url::parse(input, base).is_ok()
}

pub fn parse(
    input: &str,
    parse_query_string: bool,
    slashes_denote_host: bool,
) -> NodeResult<LegacyUrlObject> {
    if input.contains('\0') {
        return Err(NodeError::new("ERR_INVALID_URL", "URL input contains a null character"));
    }

    if parse_query_string {
        return Err(NodeError::new(
            "ERR_UNSUPPORTED_OPERATION",
            "legacy URL query-object parsing is not part of the closed runtime contract",
        ));
    }
    let (before_hash, hash_text) = split_once_with_prefix(input, '#');
    let (before_query, search_text) = split_once_with_prefix(before_hash, '?');
    let hash = (!hash_text.is_empty()).then(|| hash_text.to_string());
    let search = (!search_text.is_empty()).then(|| search_text.to_string());
    let query = search
        .as_deref()
        .map(|value| value.strip_prefix('?').unwrap_or(value).to_string());

    let (protocol_text, mut remainder) = split_scheme(before_query);
    let protocol = (!protocol_text.is_empty()).then_some(protocol_text);
    let has_authority = remainder.starts_with("//") && (protocol.is_some() || slashes_denote_host);
    let mut auth = None;
    let mut host = None;
    let mut hostname = None;
    let mut port = None;
    let pathname: Option<String>;
    if has_authority {
        remainder = remainder.strip_prefix("//").unwrap_or(remainder);
        let slash_index = remainder.find('/');
        let mut authority = slash_index.map_or(remainder, |index| &remainder[..index]);
        pathname = Some(slash_index.map_or("/", |index| &remainder[index..]).to_string());
        if let Some(at_index) = authority.rfind('@') {
            auth = Some(authority[..at_index].to_string());
            authority = &authority[at_index + 1..];
        }
        host = Some(authority.to_string());
        let (hostname_text, port_text) = split_host(authority);
        hostname = Some(hostname_text);
        port = (!port_text.is_empty()).then_some(port_text);
    } else {
        pathname = (!remainder.is_empty()).then(|| remainder.to_string());
    }
    let path = if pathname.is_none() && search.is_none() {
        None
    } else {
        Some(format!(
            "{}{}",
            pathname.as_deref().unwrap_or(""),
            search.as_deref().unwrap_or(""),
        ))
    };
    let mut parsed = LegacyUrlObject {
        href: None,
        protocol,
        slashes: has_authority.then_some(true),
        auth,
        host,
        port,
        hostname,
        hash,
        search,
        query,
        pathname,
        path,
    };
    parsed.href = Some(format(&parsed));
    Ok(parsed)
}

fn split_once_with_prefix(input: &str, separator: char) -> (&str, &str) {
    input.find(separator).map_or((input, ""), |index| (&input[..index], &input[index..]))
}

fn split_scheme(input: &str) -> (String, &str) {
    let Some(colon_index) = input.find(':') else {
        return (String::new(), input);
    };
    let scheme = &input[..colon_index];
    let mut characters = scheme.chars();
    let Some(first) = characters.next() else {
        return (String::new(), input);
    };
    if !first.is_ascii_alphabetic()
        || characters.any(|character| !character.is_ascii_alphanumeric() && !matches!(character, '+' | '-' | '.'))
    {
        return (String::new(), input);
    }
    (format!("{}:", scheme.to_ascii_lowercase()), &input[colon_index + 1..])
}

fn split_host(authority: &str) -> (String, String) {
    if authority.starts_with('[') {
        if let Some(close_bracket) = authority.find(']') {
            let hostname = authority[..=close_bracket].to_string();
            let port = authority
                .get(close_bracket + 1..)
                .and_then(|suffix| suffix.strip_prefix(':'))
                .unwrap_or("")
                .to_string();
            return (hostname, port);
        }
    }
    if let Some(colon_index) = authority.rfind(':') {
        if authority[..colon_index].contains(':') {
            return (authority.to_string(), String::new());
        }
        return (authority[..colon_index].to_string(), authority[colon_index + 1..].to_string());
    }
    (authority.to_string(), String::new())
}

/// Parses `input` into a legacy URL object without query-string expansion
/// and without slash-denoted host handling (`parse_query_string = false`,
/// `slashes_denote_host = false`).
pub fn parse_legacy(input: &str) -> NodeResult<LegacyUrlObject> {
    parse(input, false, false)
}

/// Serializes a legacy URL object back into a URL string by composing its
/// components exactly as [`format`] does, so `format_legacy(&parse_legacy(x)?)`
/// round-trips the href.
pub fn format_legacy(url: &LegacyUrlObject) -> String {
    format(url)
}

pub fn format(value: &LegacyUrlObject) -> String {
    let mut protocol = value.protocol.clone().unwrap_or_default();
    if !protocol.is_empty() && !protocol.ends_with(':') {
        protocol.push(':');
    }
    let host = value.host.clone().filter(|value| !value.is_empty()).unwrap_or_else(|| {
        let mut host = value.hostname.clone().unwrap_or_default();
        if let Some(port) = value.port.as_deref().filter(|port| !port.is_empty()) {
            host.push(':');
            host.push_str(port);
        }
        host
    });
    let slashes = value.slashes == Some(true)
        || (is_slashed_protocol(&protocol) && (!host.is_empty() || protocol == "file:"));
    let mut result = String::new();
    result.push_str(&protocol);
    if slashes {
        result.push_str("//");
    }
    if let Some(auth) = value.auth.as_deref().filter(|value| !value.is_empty()) {
        result.push_str(auth);
        result.push('@');
    }
    result.push_str(&host);
    let pathname = value.pathname.as_deref().unwrap_or("");
    if slashes && !host.is_empty() && !pathname.is_empty() && !pathname.starts_with('/') {
        result.push('/');
    }
    result.push_str(pathname);
    push_prefixed(&mut result, value.search.as_deref(), '?');
    push_prefixed(&mut result, value.hash.as_deref(), '#');
    result
}

fn is_slashed_protocol(protocol: &str) -> bool {
    matches!(protocol, "http:" | "https:" | "ftp:" | "gopher:" | "file:" | "ws:" | "wss:")
}

fn push_prefixed(result: &mut String, value: Option<&str>, prefix: char) {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return;
    };
    if !value.starts_with(prefix) {
        result.push(prefix);
    }
    result.push_str(value);
}

pub fn resolve(from: &str, to: &str) -> NodeResult<String> {
    Ok(Url::parse(to, Some(from))?.href())
}

pub fn domain_to_ascii(domain: &str) -> String {
    punycode::to_ascii(domain)
}

pub fn domain_to_unicode(domain: &str) -> String {
    punycode::to_unicode(domain)
}

pub fn url_to_http_options(url: &Url) -> HttpOptions {
    HttpOptions {
        protocol: url.protocol(),
        hostname: url.hostname(),
        port: url.port().parse::<u16>().ok(),
        path: format!("{}{}", url.pathname(), url.search()),
        auth: match (url.username.is_empty(), url.password.is_empty()) {
            (true, _) => String::new(),
            (false, true) => url.username.clone(),
            (false, false) => format!("{}:{}", url.username, url.password),
        },
    }
}

pub fn path_to_file_url(path: &str) -> Url {
    let mut pathname = path.replace('\\', "/");
    if !pathname.starts_with('/') {
        pathname = format!("/{pathname}");
    }
    let pathname = percent_encode_file_path(&pathname);
    Url {
        href: format!("file://{pathname}"),
        protocol: "file:".to_string(),
        username: String::new(),
        password: String::new(),
        host: String::new(),
        hostname: String::new(),
        port: String::new(),
        pathname,
        search: String::new(),
        hash: String::new(),
    }
}

pub fn file_url_to_path(url: &Url) -> NodeResult<String> {
    if url.protocol() != "file:" {
        return Err(NodeError::new(
            "ERR_INVALID_URL_SCHEME",
            "expected file: URL",
        ));
    }
    percent_decode_file_path(&url.pathname)
}

fn percent_encode_file_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            encoded.push(char::from(b"0123456789ABCDEF"[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

fn percent_decode_file_path(path: &str) -> NodeResult<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let Some(high) = bytes.get(index + 1).and_then(|value| hex_digit(*value)) else {
            return Err(NodeError::new(
                "ERR_INVALID_FILE_URL_PATH",
                "file URL pathname contains an incomplete percent escape",
            ));
        };
        let Some(low) = bytes.get(index + 2).and_then(|value| hex_digit(*value)) else {
            return Err(NodeError::new(
                "ERR_INVALID_FILE_URL_PATH",
                "file URL pathname contains an invalid percent escape",
            ));
        };
        decoded.push((high << 4) | low);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|_| {
        NodeError::new(
            "ERR_INVALID_FILE_URL_PATH",
            "file URL pathname is not valid UTF-8",
        )
    })
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
