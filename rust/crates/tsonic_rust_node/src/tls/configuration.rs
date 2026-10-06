fn client_config(reject_unauthorized: bool, ca: Vec<String>) -> NodeResult<rustls::ClientConfig> {
    if !reject_unauthorized {
        return Ok(rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertificateVerification::new()))
            .with_no_client_auth());
    }
    let mut roots =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for pem in ca {
        for certificate in parse_certificates(&pem)? {
            roots.add(certificate).map_err(map_tls_error)?;
        }
    }
    Ok(rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

fn server_config(options: &SourceServerOptions) -> NodeResult<rustls::ServerConfig> {
    let certificate = options.cert.as_deref().ok_or_else(|| {
        NodeError::new("ERR_TLS_CERT_REQUIRED", "TLS server options require cert")
    })?;
    let key = options
        .key
        .as_deref()
        .ok_or_else(|| NodeError::new("ERR_TLS_KEY_REQUIRED", "TLS server options require key"))?;
    let certificates = parse_certificates(certificate)?;
    if certificates.is_empty() {
        return Err(NodeError::new(
            "ERR_TLS_CERT_REQUIRED",
            "TLS certificate chain is empty",
        ));
    }
    let private_key = parse_private_key(key)?;
    let builder = rustls::ServerConfig::builder();
    let mut config = if options.request_cert.unwrap_or(false) {
        let mut roots = rustls::RootCertStore::empty();
        for pem in source_string_array(options.ca.clone()) {
            for certificate in parse_certificates(&pem)? {
                roots.add(certificate).map_err(map_tls_error)?;
            }
        }
        if roots.is_empty() {
            return Err(NodeError::new(
                "ERR_TLS_CLIENT_CA_REQUIRED",
                "requestCert requires at least one client certificate authority",
            ));
        }
        let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots));
        let verifier = if options.reject_unauthorized.unwrap_or(true) {
            verifier.build()
        } else {
            verifier.allow_unauthenticated().build()
        }
        .map_err(|error| NodeError::new("ERR_TLS_CERT", error.to_string()))?;
        builder
            .with_client_cert_verifier(verifier)
            .with_single_cert(certificates, private_key)
            .map_err(map_tls_error)?
    } else {
        builder
            .with_no_client_auth()
            .with_single_cert(certificates, private_key)
            .map_err(map_tls_error)?
    };
    config.alpn_protocols = options
        .alpn_protocols
        .clone()
        .map(|values| values.values())
        .unwrap_or_default()
        .into_iter()
        .map(String::into_bytes)
        .collect();
    Ok(config)
}

fn parse_certificates(pem: &str) -> NodeResult<Vec<CertificateDer<'static>>> {
    rustls_pemfile::certs(&mut pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_io_error)
}

fn parse_private_key(pem: &str) -> NodeResult<PrivateKeyDer<'static>> {
    rustls_pemfile::private_key(&mut pem.as_bytes())
        .map_err(map_io_error)?
        .ok_or_else(|| NodeError::new("ERR_TLS_KEY_REQUIRED", "TLS private key is empty"))
}

fn source_string_array(value: Option<tsonic_rust_js::JsArray<String>>) -> Vec<String> {
    value.map(|values| values.values()).unwrap_or_default()
}

fn source_port(value: impl tsonic_rust_runtime::conversions::IntegerInput<u16>) -> NodeResult<u16> {
    value.checked_integer().ok_or_else(|| {
        NodeError::new(
            "ERR_SOCKET_BAD_PORT",
            "port must be an unsigned 16-bit integer",
        )
    })
}

fn map_io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_TLS_IO", error.to_string())
}

#[cfg(test)]
mod numeric_bounds {
    #[test]
    fn timeout_retains_the_complete_native_domain() {
        for value in [0, 9_007_199_254_740_993, u64::MAX] {
            let options = super::SourceConnectOptions {
                timeout: Some(value),
                ..Default::default()
            };
            let prepared = super::PreparedClientConnection::new(options).unwrap();
            assert_eq!(prepared.timeout.unwrap().as_millis(), u128::from(value));
        }
    }
}

fn map_tls_error(error: rustls::Error) -> NodeError {
    NodeError::new("ERR_TLS_HANDSHAKE", error.to_string())
}

fn connecting_error() -> NodeError {
    NodeError::new(
        "ERR_SOCKET_CONNECTING",
        "TLS socket operation requires the secure connection callback to complete",
    )
}

fn io_connecting_error() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::WouldBlock, connecting_error())
}

fn io_node_error(error: &NodeError) -> std::io::Error {
    std::io::Error::other(error.clone())
}

#[derive(Debug)]
struct NoCertificateVerification(Arc<rustls::crypto::CryptoProvider>);

impl NoCertificateVerification {
    fn new() -> Self {
        Self(Arc::new(rustls::crypto::ring::default_provider()))
    }
}

impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signed: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            signed,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signed: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            signed,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
