struct ZlibState<Codec> {
    data: ZlibStateData,
    codec: Option<Codec>,
}

fn codec_state<Codec>(
    data: ZlibStateData,
    codec: Option<Codec>,
) -> std::rc::Rc<std::cell::RefCell<ZlibState<Codec>>> {
    std::rc::Rc::new(std::cell::RefCell::new(ZlibState { data, codec }))
}

macro_rules! codec_storage {
    ($error:ident; $($variant:ident($codec:ty)),+ $(,)?) => {
        enum ZlibStorage<$error: 'static> {
            $($variant(std::rc::Rc<std::cell::RefCell<ZlibState<$codec>>>)),+
        }

        impl<$error: 'static> Clone for ZlibStorage<$error> {
            fn clone(&self) -> Self {
                match self {
                    $(Self::$variant(state) => Self::$variant(std::rc::Rc::clone(state))),+
                }
            }
        }

        impl<$error: 'static> ZlibStorage<$error> {
            fn borrow(&self) -> std::cell::Ref<'_, ZlibStateData> {
                match self {
                    $(Self::$variant(state) => std::cell::Ref::map(state.borrow(), |state| &state.data)),+
                }
            }

            fn borrow_mut(&self) -> std::cell::RefMut<'_, ZlibStateData> {
                match self {
                    $(Self::$variant(state) => std::cell::RefMut::map(state.borrow_mut(), |state| &mut state.data)),+
                }
            }

            fn ptr_eq(&self, other: &Self) -> bool {
                match (self, other) {
                    $((Self::$variant(left), Self::$variant(right)) => std::rc::Rc::ptr_eq(left, right)),+,
                    _ => false,
                }
            }

            fn flush(&self, readable: &crate::stream::Readable<$error>) -> Result<(), $error>
            where
                $error: From<NodeError>,
            {
                match self {
                    $(Self::$variant(state) => flush_codec_state(state, readable)),+
                }
            }

            fn backend(
                &self,
                readable: &crate::stream::Readable<$error>,
            ) -> std::rc::Rc<dyn crate::stream::WritableBackend<$error>>
            where
                $error: From<NodeError>,
            {
                match self {
                    $(Self::$variant(state) => std::rc::Rc::new(ZlibBackend {
                        state: std::rc::Rc::clone(state),
                        readable: readable.clone(),
                    })),+
                }
            }
        }
    };
}

codec_storage! {
    E;
    Deflate(ZlibWriteEncoder<ZlibSink<E>>),
    Inflate(ZlibWriteDecoder<ZlibSink<E>>),
    Gzip(GzWriteEncoder<ZlibSink<E>>),
    Gunzip(GzWriteDecoder<ZlibSink<E>>),
    DeflateRaw(DeflateWriteEncoder<ZlibSink<E>>),
    InflateRaw(DeflateWriteDecoder<ZlibSink<E>>),
    BrotliCompress(brotli::CompressorWriter<ZlibSink<E>>),
    BrotliDecompress(brotli::DecompressorWriter<ZlibSink<E>>),
    Unzip(UnsupportedStreamingCodec),
}

trait StreamingCodec: Write + Sized {
    fn finish(self) -> std::io::Result<()>;
}

macro_rules! finish_flate_codecs {
    ($($codec:ident),+ $(,)?) => {
        $(impl<E: 'static> StreamingCodec for $codec<ZlibSink<E>> {
            fn finish(mut self) -> std::io::Result<()> {
                self.try_finish()
            }
        })+
    };
}

finish_flate_codecs! {
    ZlibWriteEncoder,
    ZlibWriteDecoder,
    GzWriteEncoder,
    GzWriteDecoder,
    DeflateWriteEncoder,
    DeflateWriteDecoder,
}

impl<E: 'static> StreamingCodec for brotli::CompressorWriter<ZlibSink<E>> {
    fn finish(self) -> std::io::Result<()> {
        let output = self.get_ref().output.clone();
        let _ = self.into_inner();
        let terminal_error = output.borrow().terminal_error.clone();
        match terminal_error {
            Some(message) => Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                message,
            )),
            None => Ok(()),
        }
    }
}

impl<E: 'static> StreamingCodec for brotli::DecompressorWriter<ZlibSink<E>> {
    fn finish(mut self) -> std::io::Result<()> {
        self.close()
    }
}

enum UnsupportedStreamingCodec {}

impl Write for UnsupportedStreamingCodec {
    fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
        match *self {}
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match *self {}
    }
}

impl StreamingCodec for UnsupportedStreamingCodec {
    fn finish(self) -> std::io::Result<()> {
        match self {}
    }
}

fn write_codec_state<Codec: StreamingCodec>(
    state: &std::rc::Rc<std::cell::RefCell<ZlibState<Codec>>>,
    input: &Buffer,
) -> NodeResult<()> {
    let mut state = state.borrow_mut();
    if state.data.closed {
        return Err(NodeError::new(
            "ERR_STREAM_WRITE_AFTER_END",
            "compression stream is closed",
        ));
    }
    state.data.bytes_written = state.data.bytes_written.saturating_add(input.len());
    let codec = state
        .codec
        .as_mut()
        .ok_or_else(unsupported_streaming_codec)?;
    input
        .with_bytes(|bytes| codec.write_all(bytes))
        .map_err(map_zlib_error)
}

fn finish_codec_state<Codec: StreamingCodec>(
    state: &std::rc::Rc<std::cell::RefCell<ZlibState<Codec>>>,
) -> NodeResult<()> {
    let codec = {
        let mut state = state.borrow_mut();
        if state.data.closed {
            None
        } else {
            let codec = state.codec.take().ok_or_else(unsupported_streaming_codec)?;
            state.data.closed = true;
            Some(codec)
        }
    };
    if let Some(codec) = codec {
        codec.finish().map_err(map_zlib_error)?;
    }
    Ok(())
}

fn flush_codec_state<E: From<NodeError> + 'static, Codec: StreamingCodec>(
    state: &std::rc::Rc<std::cell::RefCell<ZlibState<Codec>>>,
    readable: &crate::stream::Readable<E>,
) -> Result<(), E> {
    let native = state
        .borrow_mut()
        .codec
        .as_mut()
        .map_or(Ok(()), Write::flush);
    native.map_err(map_zlib_error)?;
    publish_codec_output(state, readable)
}

fn destroy_codec_state<Codec>(state: &std::rc::Rc<std::cell::RefCell<ZlibState<Codec>>>) {
    let mut state = state.borrow_mut();
    state.data.closed = true;
    state.codec = None;
    let mut output = state.data.output.borrow_mut();
    output.pending.clear();
    output.pending_bytes = 0;
}

#[cfg(test)]
mod codec_storage_tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn codec_state_capacity_follows_the_selected_native_codec() {
        type Sink = ZlibSink<NodeError>;
        let absent = size_of::<ZlibState<UnsupportedStreamingCodec>>();
        let deflate = size_of::<ZlibState<ZlibWriteEncoder<Sink>>>();
        let gzip = size_of::<ZlibState<GzWriteEncoder<Sink>>>();
        let gunzip = size_of::<ZlibState<GzWriteDecoder<Sink>>>();
        let brotli_encoder = size_of::<ZlibState<brotli::CompressorWriter<Sink>>>();
        let brotli_decoder = size_of::<ZlibState<brotli::DecompressorWriter<Sink>>>();
        assert_eq!(size_of::<Option<UnsupportedStreamingCodec>>(), 0);
        assert_eq!(absent, size_of::<ZlibStateData>());
        assert!(absent < deflate);
        assert!(deflate < gzip);
        assert!(gzip < gunzip);
        assert!(gunzip < brotli_decoder);
        assert!(brotli_decoder < brotli_encoder);
        assert_eq!(size_of::<ZlibStorage<NodeError>>(), 2 * size_of::<usize>());
    }
}
