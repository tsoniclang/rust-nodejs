fn parse_algorithm(algorithm: &str) -> NodeResult<DigestAlgorithm> {
    match algorithm.to_ascii_lowercase().as_str() {
        "md5" => Ok(DigestAlgorithm::Md5),
        "sha1" | "sha-1" => Ok(DigestAlgorithm::Sha1),
        "sha256" | "sha-256" => Ok(DigestAlgorithm::Sha256),
        "sha384" | "sha-384" => Ok(DigestAlgorithm::Sha384),
        "sha512" | "sha-512" => Ok(DigestAlgorithm::Sha512),
        other => Err(NodeError::new(
            "ERR_CRYPTO_UNSUPPORTED_ALGORITHM",
            format!("unsupported hash algorithm `{other}`"),
        )),
    }
}

#[derive(Debug, Clone)]
enum IncrementalDigest {
    Md5(Md5),
    Sha1(sha1::Sha1),
    Sha256(Sha256),
    Sha384(Sha384),
    Sha512(Sha512),
}

impl IncrementalDigest {
    fn new(algorithm: DigestAlgorithm) -> Self {
        match algorithm {
            DigestAlgorithm::Md5 => Self::Md5(Md5::new()),
            DigestAlgorithm::Sha1 => Self::Sha1(sha1::Sha1::new()),
            DigestAlgorithm::Sha256 => Self::Sha256(Sha256::new()),
            DigestAlgorithm::Sha384 => Self::Sha384(Sha384::new()),
            DigestAlgorithm::Sha512 => Self::Sha512(Sha512::new()),
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Md5(state) => Digest::update(state, bytes),
            Self::Sha1(state) => Digest::update(state, bytes),
            Self::Sha256(state) => Digest::update(state, bytes),
            Self::Sha384(state) => Digest::update(state, bytes),
            Self::Sha512(state) => Digest::update(state, bytes),
        }
    }

    fn finish(self) -> Vec<u8> {
        match self {
            Self::Md5(state) => state.finalize().to_vec(),
            Self::Sha1(state) => state.finalize().to_vec(),
            Self::Sha256(state) => state.finalize().to_vec(),
            Self::Sha384(state) => state.finalize().to_vec(),
            Self::Sha512(state) => state.finalize().to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
enum IncrementalHmac {
    Md5(hmac::Hmac<Md5>),
    Sha1(hmac::Hmac<sha1::Sha1>),
    Sha256(hmac::Hmac<Sha256>),
    Sha384(hmac::Hmac<Sha384>),
    Sha512(hmac::Hmac<Sha512>),
}

impl IncrementalHmac {
    fn new(algorithm: DigestAlgorithm, key: &[u8]) -> NodeResult<Self> {
        let invalid_key = |_| NodeError::new("ERR_CRYPTO_INVALID_KEY", "invalid HMAC key");
        match algorithm {
            DigestAlgorithm::Md5 => <hmac::Hmac<Md5> as hmac::Mac>::new_from_slice(key)
                .map(Self::Md5)
                .map_err(invalid_key),
            DigestAlgorithm::Sha1 => <hmac::Hmac<sha1::Sha1> as hmac::Mac>::new_from_slice(key)
                .map(Self::Sha1)
                .map_err(invalid_key),
            DigestAlgorithm::Sha256 => <hmac::Hmac<Sha256> as hmac::Mac>::new_from_slice(key)
                .map(Self::Sha256)
                .map_err(invalid_key),
            DigestAlgorithm::Sha384 => <hmac::Hmac<Sha384> as hmac::Mac>::new_from_slice(key)
                .map(Self::Sha384)
                .map_err(invalid_key),
            DigestAlgorithm::Sha512 => <hmac::Hmac<Sha512> as hmac::Mac>::new_from_slice(key)
                .map(Self::Sha512)
                .map_err(invalid_key),
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Md5(state) => hmac::Mac::update(state, bytes),
            Self::Sha1(state) => hmac::Mac::update(state, bytes),
            Self::Sha256(state) => hmac::Mac::update(state, bytes),
            Self::Sha384(state) => hmac::Mac::update(state, bytes),
            Self::Sha512(state) => hmac::Mac::update(state, bytes),
        }
    }

    fn finish(self) -> Vec<u8> {
        match self {
            Self::Md5(state) => hmac::Mac::finalize(state).into_bytes().to_vec(),
            Self::Sha1(state) => hmac::Mac::finalize(state).into_bytes().to_vec(),
            Self::Sha256(state) => hmac::Mac::finalize(state).into_bytes().to_vec(),
            Self::Sha384(state) => hmac::Mac::finalize(state).into_bytes().to_vec(),
            Self::Sha512(state) => hmac::Mac::finalize(state).into_bytes().to_vec(),
        }
    }
}

fn digest_bytes(algorithm: DigestAlgorithm, input: &[u8]) -> Vec<u8> {
    let mut state = IncrementalDigest::new(algorithm);
    state.update(input);
    state.finish()
}

fn hmac_digest_buffer(algorithm: DigestAlgorithm, key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut state = IncrementalHmac::new(algorithm, key).expect("HMAC accepts keys of any length");
    state.update(data);
    state.finish()
}

fn digest_len(algorithm: DigestAlgorithm) -> usize {
    match algorithm {
        DigestAlgorithm::Md5 => 16,
        DigestAlgorithm::Sha1 => 20,
        DigestAlgorithm::Sha256 => 32,
        DigestAlgorithm::Sha384 => 48,
        DigestAlgorithm::Sha512 => 64,
    }
}
