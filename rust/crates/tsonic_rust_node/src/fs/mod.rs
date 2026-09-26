include!("types_core.rs");
include!("options.rs");
include!("streams.rs");
include!("dir.rs");
include!("sync_file.rs");
include!("sync_dir.rs");
include!("sync_fd.rs");
include!("descriptor_io.rs");
include!("descriptor_abi.rs");
include!("path_abi.rs");
include!("stream_api.rs");
include!("callbacks.rs");
include!("watch_glob.rs");
include!("metadata.rs");
include!("constants_errors.rs");

#[cfg(test)]
mod numeric_bounds {
    #[test]
    fn stream_integer_bounds_do_not_saturate() {
        let source = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        for options in [
            super::ReadStreamOptions {
                start: Some(1),
                end: Some(0),
                ..Default::default()
            },
            super::ReadStreamOptions {
                start: Some(0),
                end: Some(u64::MAX),
                ..Default::default()
            },
            super::ReadStreamOptions {
                high_water_mark: Some(0),
                ..Default::default()
            },
        ] {
            let result = super::ReadStream::from_file(
                source.to_owned(),
                std::fs::File::open(source).unwrap(),
                &options,
            );
            assert_eq!(
                result.err().expect("invalid range was accepted").code,
                "ERR_OUT_OF_RANGE"
            );
        }
        let options = super::ReadStreamOptions {
            end: Some(u64::MAX - 1),
            high_water_mark: Some(usize::MAX),
            ..Default::default()
        };
        let result = super::ReadStream::from_file(
            source.to_owned(),
            std::fs::File::open(source).unwrap(),
            &options,
        )
        .unwrap();
        let state = result.state.borrow();
        assert_eq!(state.remaining, Some(u64::MAX));
        assert_eq!(state.chunk_size, usize::MAX);
    }

    #[test]
    fn file_time_rejects_the_exclusive_signed_upper_bound() {
        let limit = (i64::MAX as i128 + 1) as f64;
        assert!(super::file_time_from_seconds(limit).is_err());
        assert_eq!(
            super::file_time_from_seconds(limit.next_down())
                .unwrap()
                .unix_seconds(),
            limit.next_down() as i64
        );
        assert_eq!(
            super::file_time_from_seconds(i64::MIN as f64)
                .unwrap()
                .unix_seconds(),
            i64::MIN
        );
    }
}
