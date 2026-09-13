use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tsonic_rust_node::perf_hooks;

#[test]
fn performance_clock_includes_time_before_its_first_query_and_keeps_one_origin() {
    perf_hooks::initialize_clock();
    std::thread::sleep(Duration::from_millis(30));
    let elapsed = perf_hooks::performance_now();
    assert!(elapsed >= 30.0);
    let origin = perf_hooks::time_origin();
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        * 1000.0;
    assert!((origin + elapsed - wall).abs() < 100.0);
    perf_hooks::initialize_clock();
    assert_eq!(perf_hooks::time_origin(), origin);
    assert!(perf_hooks::performance_now() >= elapsed);
    let value = perf_hooks::performance();
    assert_eq!(value.time_origin, origin);
    assert!(value.now() >= elapsed);
}
