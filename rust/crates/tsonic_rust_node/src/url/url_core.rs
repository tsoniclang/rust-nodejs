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
    pub href: String,
    pub protocol: String,
    pub slashes: bool,
    pub auth: String,
    pub host: String,
    pub port: String,
    pub hostname: String,
    pub hash: String,
    pub search: String,
    pub query: String,
    pub pathname: String,
    pub path: String,
}

impl LegacyUrlObject {
    /// Returns the full serialized URL. Empty string when absent.
    pub fn href(&self) -> String {
        self.href.clone()
    }

    /// Returns the protocol including the trailing colon (for example
    /// `https:`). Empty string when absent.
    pub fn protocol(&self) -> String {
        self.protocol.clone()
    }

    /// Returns the host including the port when present (for example
    /// `example.com:8443`). Empty string when absent.
    pub fn host(&self) -> String {
        self.host.clone()
    }

    /// Returns the hostname without the port. Empty string when absent.
    pub fn hostname(&self) -> String {
        self.hostname.clone()
    }

    /// Returns the port as a decimal string. Empty string when absent.
    pub fn port(&self) -> String {
        self.port.clone()
    }

    /// Returns the pathname (for example `/a/b`). Empty string when absent.
    pub fn pathname(&self) -> String {
        self.pathname.clone()
    }

    /// Returns the search string including the leading `?`. Empty string
    /// when absent.
    pub fn search(&self) -> String {
        self.search.clone()
    }

    /// Returns the query string without the leading `?`. Empty string when
    /// absent.
    pub fn query(&self) -> String {
        self.query.clone()
    }

    /// Returns the fragment including the leading `#`. Empty string when
    /// absent.
    pub fn hash(&self) -> String {
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
    let url = if input.contains("://") {
        Url::parse(input, None)?
    } else if slashes_denote_host && input.starts_with("//") {
        Url::parse(&format!("http:{input}"), None)?
    } else {
        Url::parse(input, Some("http://localhost"))?
    };
    let query = url
        .search
        .strip_prefix('?')
        .unwrap_or(&url.search)
        .to_string();
    let auth = match (url.username.is_empty(), url.password.is_empty()) {
        (true, _) => String::new(),
        (false, true) => url.username.clone(),
        (false, false) => format!("{}:{}", url.username, url.password),
    };
    let query_value = if parse_query_string {
        UrlSearchParams::new(Some(&query))?.to_string()
    } else {
        query
    };
    Ok(LegacyUrlObject {
        href: url.href.clone(),
        protocol: url.protocol.clone(),
        slashes: true,
        auth,
        host: url.host.clone(),
        port: url.port.clone(),
        hostname: url.hostname.clone(),
        hash: url.hash.clone(),
        search: url.search.clone(),
        query: query_value,
        pathname: url.pathname.clone(),
        path: format!("{}{}", url.pathname, url.search),
    })
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
    let mut result = String::new();
    result.push_str(&value.protocol);
    if value.slashes {
        result.push_str("//");
    }
    if !value.auth.is_empty() {
        result.push_str(&value.auth);
        result.push('@');
    }
    result.push_str(&value.host);
    result.push_str(&value.pathname);
    result.push_str(&value.search);
    result.push_str(&value.hash);
    result
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
