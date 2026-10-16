use tsonic_rust_node::buffer::Buffer;
use tsonic_rust_node::runtime_tasks::RuntimeTasks;
use tsonic_rust_node::zlib::{self, Zlib, ZlibMode};
use tsonic_rust_node::NodeError;

#[path = "../../../../rust-runtime/tests/helpers/allocation_counts.rs"]
mod allocation_counts;

#[test]
fn selected_codec_storage_does_not_add_an_owner_allocation() {
    for (mode, expected_allocations) in [
        (ZlibMode::Deflate, 16),
        (ZlibMode::Inflate, 11),
        (ZlibMode::Gzip, 17),
        (ZlibMode::Gunzip, 12),
        (ZlibMode::DeflateRaw, 16),
        (ZlibMode::InflateRaw, 11),
        (ZlibMode::Unzip, 9),
        (ZlibMode::BrotliCompress, 16),
        (ZlibMode::BrotliDecompress, 14),
    ] {
        let tasks = RuntimeTasks::<NodeError>::new();
        let handle = tasks.handle();
        let mut codec = None;
        let allocations = allocation_counts::measure_requests(|| {
            codec = Some(Zlib::<NodeError>::new(handle, mode, None));
        });
        assert_eq!(allocations.0, expected_allocations, "{mode:?}");
        let codec = codec.unwrap();
        let mut alias = None;
        assert_eq!(
            allocation_counts::measure_requests(|| alias = Some(codec.clone())),
            (0, 0),
            "{mode:?}",
        );
        let alias = alias.unwrap();
        assert_eq!(codec, alias);
        let independent = Zlib::<NodeError>::new(tasks.handle(), mode, None);
        assert_ne!(codec, independent);
        alias.close(None::<fn()>).unwrap();
        assert!(codec.closed());
        assert!(!independent.closed());
    }
}

#[test]
fn every_selected_streaming_codec_preserves_chunked_native_bytes() {
    let plain = Buffer::from_bytes(
        (0_usize..65536)
            .map(|index| (index.wrapping_mul(17) ^ (index >> 3)) as u8)
            .collect(),
    );
    for (encoder, decoder) in [
        (ZlibMode::Deflate, ZlibMode::Inflate),
        (ZlibMode::Gzip, ZlibMode::Gunzip),
        (ZlibMode::DeflateRaw, ZlibMode::InflateRaw),
        (ZlibMode::BrotliCompress, ZlibMode::BrotliDecompress),
    ] {
        let compressed = stream(encoder, &plain);
        let decoded = match decoder {
            ZlibMode::Inflate => zlib::inflate_sync(&compressed),
            ZlibMode::Gunzip => zlib::gunzip_sync(&compressed),
            ZlibMode::InflateRaw => zlib::inflate_raw_sync(&compressed),
            ZlibMode::BrotliDecompress => zlib::brotli_decompress_sync(&compressed),
            _ => panic!("a selected compression pair must have a native decoder"),
        };
        assert_eq!(decoded.unwrap(), plain, "{encoder:?}");
        assert_eq!(stream(decoder, &compressed), plain, "{decoder:?}");
    }
}

#[test]
fn reset_replaces_only_the_selected_codec_owner() {
    for mode in [ZlibMode::Gzip, ZlibMode::Deflate, ZlibMode::BrotliCompress] {
        let tasks = RuntimeTasks::<NodeError>::new();
        let mut codec = Zlib::<NodeError>::new(tasks.handle(), mode, None);
        let original = codec.clone();
        codec.write(Buffer::from_bytes(vec![1, 2, 3])).unwrap();
        assert_eq!(original.bytes_written(), 3);
        codec.reset();
        assert_ne!(original, codec);
        assert_eq!(original.bytes_written(), 3);
        assert_eq!(codec.bytes_written(), 0);
        codec.close(None::<fn()>).unwrap();
        assert!(codec.closed());
        assert!(!original.closed());
        original.close(None::<fn()>).unwrap();
    }
}

#[test]
fn absent_streaming_codec_preserves_its_exact_native_failure_domain() {
    enum Failure {
        Native(NodeError),
    }
    impl From<NodeError> for Failure {
        fn from(error: NodeError) -> Self {
            Self::Native(error)
        }
    }
    let tasks = RuntimeTasks::<Failure>::new();
    let codec = Zlib::<Failure>::new(tasks.handle(), ZlibMode::Unzip, None);
    match codec.write(Buffer::from_bytes(vec![1])) {
        Err(Failure::Native(error)) => assert_eq!(error.code(), "ERR_UNSUPPORTED_OPERATION"),
        Ok(_) => panic!("an absent streaming codec must not accept a write"),
    }
}

fn stream(mode: ZlibMode, input: &Buffer) -> Buffer {
    let tasks = RuntimeTasks::<NodeError>::new();
    let codec = Zlib::<NodeError>::new(tasks.handle(), mode, None);
    let mut output = Vec::new();
    for start in (0..input.len()).step_by(1024) {
        codec
            .write(input.slice(start as isize, Some(input.len().min(start + 1024) as isize)))
            .unwrap();
        drain(&codec, &mut output);
    }
    codec.end().unwrap();
    drain(&codec, &mut output);
    tasks.poll().unwrap();
    assert!(codec.closed());
    assert_eq!(codec.bytes_written(), input.len());
    Buffer::from_bytes(output)
}

fn drain(codec: &Zlib<NodeError>, output: &mut Vec<u8>) {
    while let Some(chunk) = codec.read().unwrap() {
        chunk.with_bytes(|bytes| output.extend_from_slice(bytes));
    }
}
