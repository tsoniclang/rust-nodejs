use std::cell::{RefCell, RefMut};
use std::rc::Rc;

pub fn create_hash(algorithm: &str) -> NodeResult<Hash> {
    Hash::create(algorithm)
}

pub fn create_hash_with_options(algorithm: &str, options: HashOptions) -> NodeResult<Hash> {
    if options.output_length.is_some() {
        return Err(NodeError::new(
            "ERR_CRYPTO_UNSUPPORTED_OPTION",
            "hash outputLength is only valid for unsupported XOF algorithms",
        ));
    }
    create_hash(algorithm)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestResult {
    Buffer(Buffer),
    String(String),
}

#[derive(Debug)]
pub struct Hash {
    state: Rc<RefCell<HashState>>,
}

#[derive(Debug, Clone)]
struct HashState {
    digest: Option<IncrementalDigest>,
}

impl Clone for Hash {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DigestAlgorithm {
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl Hash {
    pub fn create(algorithm: &str) -> NodeResult<Self> {
        let algorithm = parse_algorithm(algorithm)?;
        Ok(Self {
            state: Rc::new(RefCell::new(HashState {
                digest: Some(IncrementalDigest::new(algorithm)),
            })),
        })
    }

    fn writable_state(&self) -> NodeResult<RefMut<'_, HashState>> {
        let state = self.state.try_borrow_mut().map_err(|_| {
            NodeError::new("ERR_CRYPTO_INVALID_STATE", "hash state is already borrowed")
        })?;
        if state.digest.is_none() {
            return Err(NodeError::new(
                "ERR_CRYPTO_HASH_FINALIZED",
                "hash state has already been finalized",
            ));
        }
        Ok(state)
    }

    pub fn update_bytes(&mut self, bytes: &[u8]) -> NodeResult<&mut Self> {
        self.writable_state()?
            .digest
            .as_mut()
            .expect("writable digest")
            .update(bytes);
        Ok(self)
    }

    pub fn update(&mut self, bytes: &[u8]) -> NodeResult<&mut Self> {
        self.update_bytes(bytes)
    }

    pub fn copy(&self) -> NodeResult<Self> {
        let state = self.state.try_borrow().map_err(|_| {
            NodeError::new("ERR_CRYPTO_INVALID_STATE", "hash state is already borrowed")
        })?;
        if state.digest.is_none() {
            return Err(NodeError::new(
                "ERR_CRYPTO_HASH_FINALIZED",
                "hash state has already been finalized",
            ));
        }
        Ok(Self {
            state: Rc::new(RefCell::new(state.clone())),
        })
    }

    pub fn update_string(&mut self, value: &str, encoding: Option<&str>) -> NodeResult<&mut Self> {
        if encoding.is_none_or(|encoding| {
            encoding.eq_ignore_ascii_case("utf8") || encoding.eq_ignore_ascii_case("utf-8")
        }) {
            return self.update_bytes(value.as_bytes());
        }
        let bytes = crate::buffer::encode_string(value, encoding)?;
        self.update_bytes(&bytes)
    }

    pub fn update_str(&mut self, value: &str) -> NodeResult<&mut Self> {
        self.update_string(value, None)
    }

    pub fn update_str_owned(&mut self, value: &str) -> NodeResult<Self> {
        self.update_str(value)?;
        Ok(self.clone())
    }

    pub fn update_buffer_owned(&mut self, value: &Buffer) -> NodeResult<Self> {
        value.with_bytes(|bytes| self.update_bytes(bytes).map(|_| ()))?;
        Ok(self.clone())
    }

    pub fn digest(self, encoding: Option<&str>) -> NodeResult<DigestResult> {
        let digest = self
            .writable_state()?
            .digest
            .take()
            .expect("writable digest");
        let bytes = digest.finish();
        match encoding {
            None => Ok(DigestResult::Buffer(Buffer::from_bytes(bytes))),
            Some(encoding) => Ok(DigestResult::String(decode_bytes(&bytes, Some(encoding))?)),
        }
    }

    pub fn digest_string(self, encoding: &str) -> NodeResult<String> {
        match self.digest(Some(encoding))? {
            DigestResult::String(value) => Ok(value),
            DigestResult::Buffer(_) => Err(NodeError::new(
                "ERR_INVALID_RETURN_VALUE",
                "hash string digest returned a buffer",
            )),
        }
    }

    pub fn digest_buffer(self) -> NodeResult<Buffer> {
        match self.digest(None)? {
            DigestResult::Buffer(value) => Ok(value),
            DigestResult::String(_) => Err(NodeError::new(
                "ERR_INVALID_RETURN_VALUE",
                "hash buffer digest returned a string",
            )),
        }
    }
}

pub fn hash(algorithm: &str, data: &[u8], encoding: Option<&str>) -> NodeResult<DigestResult> {
    let mut hash = create_hash(algorithm)?;
    hash.update_bytes(data)?;
    hash.digest(encoding)
}

pub fn hash_with_options(
    algorithm: &str,
    data: &[u8],
    options: OneShotDigestOptions,
) -> NodeResult<DigestResult> {
    if options.output_length.is_some() {
        return Err(NodeError::new(
            "ERR_CRYPTO_UNSUPPORTED_OPTION",
            "one-shot digest outputLength is only valid for unsupported XOF algorithms",
        ));
    }
    hash(algorithm, data, options.output_encoding.as_deref())
}

#[derive(Debug, Clone)]
pub struct Hmac {
    digest: IncrementalHmac,
}

impl Hmac {
    pub fn create(algorithm: &str, key: &[u8]) -> NodeResult<Self> {
        Ok(Self {
            digest: IncrementalHmac::new(parse_algorithm(algorithm)?, key)?,
        })
    }

    pub fn update_bytes(&mut self, bytes: &[u8]) {
        self.digest.update(bytes);
    }

    pub fn update(&mut self, bytes: &[u8]) -> &mut Self {
        self.update_bytes(bytes);
        self
    }

    pub fn update_string(&mut self, value: &str, encoding: Option<&str>) -> NodeResult<()> {
        if encoding.is_none_or(|encoding| {
            encoding.eq_ignore_ascii_case("utf8") || encoding.eq_ignore_ascii_case("utf-8")
        }) {
            self.update_bytes(value.as_bytes());
        } else {
            self.update_bytes(&crate::buffer::encode_string(value, encoding)?);
        }
        Ok(())
    }

    pub fn update_str(&mut self, value: &str) -> NodeResult<()> {
        self.update_string(value, None)
    }

    pub fn digest(self, encoding: Option<&str>) -> NodeResult<DigestResult> {
        encode_digest(self.digest.finish(), encoding)
    }

    pub fn digest_string(self, encoding: &str) -> NodeResult<String> {
        match self.digest(Some(encoding))? {
            DigestResult::String(value) => Ok(value),
            DigestResult::Buffer(_) => Err(NodeError::new(
                "ERR_INVALID_RETURN_VALUE",
                "hmac string digest returned a buffer",
            )),
        }
    }

    pub fn digest_buffer(self) -> NodeResult<Buffer> {
        match self.digest(None)? {
            DigestResult::Buffer(value) => Ok(value),
            DigestResult::String(_) => Err(NodeError::new(
                "ERR_INVALID_RETURN_VALUE",
                "hmac buffer digest returned a string",
            )),
        }
    }
}

pub fn create_hmac(algorithm: &str, key: &[u8]) -> NodeResult<Hmac> {
    Hmac::create(algorithm, key)
}

pub fn create_hmac_str(algorithm: &str, key: &str) -> NodeResult<Hmac> {
    create_hmac(algorithm, key.as_bytes())
}

pub fn hmac_digest(
    algorithm: &str,
    key: &[u8],
    data: &[u8],
    encoding: Option<&str>,
) -> NodeResult<DigestResult> {
    let algorithm = parse_algorithm(algorithm)?;
    hmac_digest_algorithm(algorithm, key, data, encoding)
}

fn hmac_digest_algorithm(
    algorithm: DigestAlgorithm,
    key: &[u8],
    data: &[u8],
    encoding: Option<&str>,
) -> NodeResult<DigestResult> {
    let mut digest = IncrementalHmac::new(algorithm, key)?;
    digest.update(data);
    encode_digest(digest.finish(), encoding)
}

fn encode_digest(bytes: Vec<u8>, encoding: Option<&str>) -> NodeResult<DigestResult> {
    match encoding {
        None => Ok(DigestResult::Buffer(Buffer::from_bytes(bytes))),
        Some(encoding) => Ok(DigestResult::String(decode_bytes(&bytes, Some(encoding))?)),
    }
}
