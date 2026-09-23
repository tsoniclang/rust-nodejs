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
        let unsigned_limit = (u64::MAX as u128 + 1) as f64;
        assert!(super::optional_non_negative_integer(Some(unsigned_limit), "start").is_err());
        assert_eq!(super::optional_non_negative_integer(Some(unsigned_limit.next_down()), "start").unwrap(), Some(unsigned_limit.next_down() as u64));
        let size_limit = (usize::MAX as u128 + 1) as f64;
        assert!(super::optional_positive_usize(Some(size_limit), "size").is_err());
        let maximum = size_limit.next_down().floor();
        assert_eq!(super::optional_positive_usize(Some(maximum), "size").unwrap(), Some(maximum as usize));
        assert!(super::optional_positive_usize(Some(0.0), "size").is_err());
    }

    #[test]
    fn file_time_rejects_the_exclusive_signed_upper_bound() {
        let limit = (i64::MAX as i128 + 1) as f64;
        assert!(super::file_time_from_seconds(limit).is_err());
        assert_eq!(super::file_time_from_seconds(limit.next_down()).unwrap().unix_seconds(), limit.next_down() as i64);
        assert_eq!(super::file_time_from_seconds(i64::MIN as f64).unwrap().unix_seconds(), i64::MIN);
    }
}
