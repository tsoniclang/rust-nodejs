impl Buffer {
    pub(crate) fn with_mut_bytes<Result>(
        &self,
        operation: impl FnOnce(&mut [u8]) -> Result,
    ) -> Result {
        self.view.with_mut_bytes(operation)
    }

    pub fn alloc(size: usize) -> Self {
        Self::from_bytes(vec![0; size])
    }

    pub fn alloc_with_fill(size: usize, fill: BufferValue) -> NodeResult<Self> {
        let mut buffer = Self::alloc(size);
        buffer.fill_value(fill, 0, None)?;
        Ok(buffer)
    }

    pub fn alloc_unsafe(size: usize) -> Self {
        Self::alloc(size)
    }

    pub fn alloc_unsafe_slow(size: usize) -> Self {
        Self::alloc(size)
    }

    pub fn of(items: &[u8]) -> Self {
        Self::from_bytes(items.to_vec())
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            view: tsonic_rust_js::Uint8Array::from_bytes(bytes),
        }
    }

    pub fn as_uint8_array(&self) -> tsonic_rust_js::Uint8Array {
        self.view.clone()
    }

    pub fn from_string(value: &str, encoding: Option<&str>) -> NodeResult<Self> {
        Ok(Self::from_bytes(encode_string(value, encoding)?))
    }

    pub fn from_string_enc(value: &str, encoding: &str) -> NodeResult<Self> {
        Self::from_string(value, Some(encoding))
    }

    pub fn from_array_like(values: &[u8]) -> Self {
        Self::from_bytes(values.to_vec())
    }

    pub fn from_uint8_array(value: &tsonic_rust_js::Uint8Array) -> Self {
        value.with_bytes(|bytes| Self::from_bytes(bytes.to_vec()))
    }

    pub fn from_number_array(values: &JsArray<f64>) -> Self {
        let bytes = values
            .values()
            .into_iter()
            .map(|value| value.map(to_uint8).unwrap_or(0))
            .collect();
        Self::from_bytes(bytes)
    }

    pub fn copy_bytes_from(view: &[u8], offset: usize, length: Option<usize>) -> NodeResult<Self> {
        if offset > view.len() {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "copyBytesFrom offset is outside view",
            ));
        }
        let end = offset.saturating_add(length.unwrap_or(view.len() - offset));
        if end > view.len() {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "copyBytesFrom length is outside view",
            ));
        }
        Ok(Self::from_bytes(view[offset..end].to_vec()))
    }

    pub fn byte_length(value: &str, encoding: Option<&str>) -> NodeResult<usize> {
        if matches!(encoding, None | Some("utf8" | "utf-8")) {
            return Ok(value.len());
        }
        Ok(encode_string(value, encoding)?.len())
    }

    pub fn byte_length_enc(value: &str, encoding: &str) -> NodeResult<usize> {
        Self::byte_length(value, Some(encoding))
    }

    pub fn len(&self) -> usize {
        self.view.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }

    pub(crate) fn with_bytes<T>(&self, operation: impl FnOnce(&[u8]) -> T) -> T {
        self.view.with_bytes(operation)
    }

    pub fn get(&self, index: usize) -> Option<u8> {
        if index >= self.len() {
            return None;
        }
        self.with_bytes(|bytes| bytes.get(index).copied())
    }

    pub fn read_u8(&self, index: usize) -> NodeResult<u8> {
        self.get(index)
            .ok_or_else(|| NodeError::new("ERR_OUT_OF_RANGE", "buffer index out of range"))
    }

    pub fn set(&mut self, index: usize, value: u8) -> NodeResult<()> {
        if index >= self.len() {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "buffer index out of range",
            ));
        }
        self.with_mut_bytes(|bytes| bytes[index] = value);
        Ok(())
    }

    pub fn fill(&mut self, value: u8, start: usize, end: Option<usize>) -> NodeResult<&mut Self> {
        self.fill_bytes(&[value], start, end)
    }

    pub fn fill_value(
        &mut self,
        value: BufferValue,
        start: usize,
        end: Option<usize>,
    ) -> NodeResult<&mut Self> {
        let bytes = value.into_bytes()?;
        self.fill_bytes(&bytes, start, end)
    }

    pub fn fill_string(
        &mut self,
        value: &str,
        start: usize,
        end: Option<usize>,
        encoding: Option<&str>,
    ) -> NodeResult<&mut Self> {
        self.fill_value(BufferValue::text(value, encoding), start, end)
    }

    pub fn fill_bytes(
        &mut self,
        value: &[u8],
        start: usize,
        end: Option<usize>,
    ) -> NodeResult<&mut Self> {
        let end = end.unwrap_or(self.len()).min(self.len());
        if start > end {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "buffer fill start is after end",
            ));
        }
        if value.is_empty() {
            return Ok(self);
        }
        self.with_mut_bytes(|bytes| {
            let output = &mut bytes[start..end];
            let mut filled = value.len().min(output.len());
            output[..filled].copy_from_slice(&value[..filled]);
            while filled < output.len() {
                let count = filled.min(output.len() - filled);
                output.copy_within(..count, filled);
                filled += count;
            }
        });
        Ok(self)
    }

    pub fn copy(
        &self,
        target: &Buffer,
        target_start: usize,
        source_start: usize,
        source_end: Option<usize>,
    ) -> NodeResult<usize> {
        if target_start > target.len() || source_start > self.len() {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "buffer copy range is outside buffer",
            ));
        }
        let source_end = source_end.unwrap_or(self.len()).min(self.len());
        if source_start > source_end {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "buffer copy source start is after source end",
            ));
        }
        let count = (source_end - source_start).min(target.len() - target_start);
        let source_offset = self.view.byte_offset() as usize + source_start;
        target.view.buffer().copy_bytes_from(
            target.view.byte_offset() as usize + target_start,
            &self.view.buffer(), source_offset..source_offset + count,
        );
        Ok(count)
    }

    pub fn copy_to_slice(
        &self,
        target: &mut [u8],
        target_start: usize,
        source_start: usize,
        source_end: Option<usize>,
    ) -> NodeResult<usize> {
        if target_start > target.len() || source_start > self.len() {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "buffer copy range is outside buffer",
            ));
        }
        let source_end = source_end.unwrap_or(self.len()).min(self.len());
        if source_start > source_end {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "buffer copy source start is after source end",
            ));
        }
        let count = (source_end - source_start).min(target.len() - target_start);
        self.with_bytes(|bytes| target[target_start..target_start + count]
            .copy_from_slice(&bytes[source_start..source_start + count]));
        Ok(count)
    }

    pub fn slice(&self, start: isize, end: Option<isize>) -> Self {
        self.view(start, end)
    }

    pub fn subarray(&self, start: isize, end: Option<isize>) -> Self {
        self.view(start, end)
    }

    pub fn to_string(&self, encoding: Option<&str>) -> NodeResult<String> {
        self.with_bytes(|bytes| decode_bytes(bytes, encoding))
    }

    pub fn to_string_enc(&self, encoding: &str) -> NodeResult<String> {
        self.to_string(Some(encoding))
    }

    pub fn to_json(&self) -> JsValue {
        let values = self.with_bytes(|bytes| bytes.iter()
            .map(|byte| JsValue::Number(f64::from(*byte))).collect::<Vec<_>>());
        JsValue::object(JsObject::from_pairs([
            ("type", JsValue::String(("Buffer").to_owned())),
            ("data", JsValue::from(values)),
        ]))
    }

    fn with_pair<Result>(&self, other: &Buffer, operation: impl FnOnce(&[u8], &[u8]) -> Result) -> Result {
        let left_start = self.view.byte_offset() as usize;
        let right_start = other.view.byte_offset() as usize;
        self.view.buffer().with_byte_ranges(left_start..left_start + self.len(),
            &other.view.buffer(), right_start..right_start + other.len(), operation)
    }

    pub fn equals(&self, other: &Buffer) -> bool {
        self.with_pair(other, |left, right| left == right)
    }

    pub fn compare(&self, other: &Buffer) -> i32 {
        match self.with_pair(other, <[u8]>::cmp) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    pub fn includes(&self, needle: &[u8], byte_offset: isize) -> bool {
        self.index_of(needle, byte_offset).is_some()
    }

    pub fn includes_value(&self, needle: BufferValue, byte_offset: isize) -> NodeResult<bool> {
        Ok(self.index_of_value(needle, byte_offset)?.is_some())
    }

    pub fn includes_string(
        &self,
        needle: &str,
        byte_offset: isize,
        encoding: Option<&str>,
    ) -> NodeResult<bool> {
        self.includes_value(BufferValue::text(needle, encoding), byte_offset)
    }

    pub fn index_of(&self, needle: &[u8], byte_offset: isize) -> Option<usize> {
        if needle.is_empty() {
            return Some(normalize_search_start(self.len(), byte_offset));
        }
        let start = normalize_search_start(self.len(), byte_offset);
        self.with_bytes(|bytes| memchr::memmem::find(&bytes[start..], needle).map(|index| index + start))
    }

    pub fn index_of_value(
        &self,
        needle: BufferValue,
        byte_offset: isize,
    ) -> NodeResult<Option<usize>> {
        Ok(self.index_of(&needle.into_bytes()?, byte_offset))
    }

    pub fn index_of_string(
        &self,
        needle: &str,
        byte_offset: isize,
        encoding: Option<&str>,
    ) -> NodeResult<Option<usize>> {
        self.index_of_value(BufferValue::text(needle, encoding), byte_offset)
    }

    pub fn last_index_of(&self, needle: &[u8], byte_offset: Option<isize>) -> Option<usize> {
        if needle.is_empty() {
            return Some(byte_offset.map_or(self.len(), |offset| {
                normalize_search_start(self.len(), offset)
            }));
        }
        if needle.len() > self.len() {
            return None;
        }
        let max_start = self.len() - needle.len();
        let start = byte_offset
            .map(|offset| normalize_search_start(self.len(), offset).min(max_start))
            .unwrap_or(max_start);
        self.with_bytes(|bytes| memchr::memmem::rfind(&bytes[..start + needle.len()], needle))
    }

    pub fn last_index_of_value(
        &self,
        needle: BufferValue,
        byte_offset: Option<isize>,
    ) -> NodeResult<Option<usize>> {
        Ok(self.last_index_of(&needle.into_bytes()?, byte_offset))
    }

    pub fn last_index_of_string(
        &self,
        needle: &str,
        byte_offset: Option<isize>,
        encoding: Option<&str>,
    ) -> NodeResult<Option<usize>> {
        self.last_index_of_value(BufferValue::text(needle, encoding), byte_offset)
    }

    pub fn concat(buffers: &JsArray<Buffer>) -> NodeResult<Buffer> {
        let buffers = buffers.values();
        if buffers.iter().any(Option::is_none) {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_TYPE",
                "Buffer.concat list must not contain array holes",
            ));
        }
        Ok(Self::concat_dense(
            &buffers.into_iter().flatten().collect::<Vec<_>>(),
        ))
    }

    pub(crate) fn concat_dense(buffers: &[Buffer]) -> Buffer {
        let mut out = Vec::with_capacity(buffers.iter().map(Buffer::len).sum());
        for buffer in buffers {
            buffer.with_bytes(|bytes| out.extend_from_slice(bytes));
        }
        Buffer::from_bytes(out)
    }

    pub fn concat_with_total_length(
        buffers: &JsArray<Buffer>,
        total_length: usize,
    ) -> NodeResult<Buffer> {
        let buffers = buffers.values();
        if buffers.iter().any(Option::is_none) {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_TYPE",
                "Buffer.concat list must not contain array holes",
            ));
        }
        Ok(Self::concat_dense_with_total_length(
            &buffers.into_iter().flatten().collect::<Vec<_>>(),
            total_length,
        ))
    }

    fn concat_dense_with_total_length(buffers: &[Buffer], total_length: usize) -> Buffer {
        let mut out = Vec::with_capacity(total_length);
        for buffer in buffers {
            buffer.with_bytes(|bytes| out.extend_from_slice(&bytes[..bytes.len().min(total_length - out.len())]));
            if out.len() >= total_length {
                out.truncate(total_length);
                return Buffer::from_bytes(out);
            }
        }
        out.resize(total_length, 0);
        Buffer::from_bytes(out)
    }

    pub fn write(
        &mut self,
        value: &str,
        offset: usize,
        length: Option<usize>,
        encoding: Option<&str>,
    ) -> NodeResult<usize> {
        if offset > self.len() {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "buffer write offset out of range",
            ));
        }
        let bytes = encode_string(value, encoding)?;
        let count = bytes
            .len()
            .min(length.unwrap_or(bytes.len()))
            .min(self.len() - offset);
        self.write_exact(offset, &bytes[..count])?;
        Ok(count)
    }
}

fn to_uint8(value: f64) -> u8 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    value.trunc().rem_euclid(256.0) as u8
}
