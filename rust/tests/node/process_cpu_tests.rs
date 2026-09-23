use std::time::{Duration, Instant};
use tsonic_rust_node::process::{cpu_usage, CpuUsage};

#[test]
#[cfg(unix)]
fn process_cpu_matches_os_user_and_system_counters() {
    use nix::sys::{
        resource::{getrusage, UsageWho},
        time::TimeValLike,
    };
    let before = getrusage(UsageWho::RUSAGE_SELF).unwrap();
    let actual = cpu_usage(None).unwrap();
    let after = getrusage(UsageWho::RUSAGE_SELF).unwrap();
    assert!(actual.user >= before.user_time().num_microseconds());
    assert!(actual.user <= after.user_time().num_microseconds());
    assert!(actual.system >= before.system_time().num_microseconds());
    assert!(actual.system <= after.system_time().num_microseconds());
}

#[test]
#[cfg(unix)]
fn process_cpu_sleep_is_not_reported_as_cpu_and_deltas_retain_sign() {
    let before = cpu_usage(None).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let sleeping = cpu_usage(Some(before)).unwrap();
    assert!(sleeping.user + sleeping.system < 50_000);
    let before = cpu_usage(None).unwrap();
    let started = Instant::now();
    let mut value = 1_u64;
    while started.elapsed() < Duration::from_millis(20) {
        value = std::hint::black_box(value.wrapping_mul(7).wrapping_add(3));
    }
    let working = cpu_usage(Some(before)).unwrap();
    assert!(working.user + working.system > 0);
    let future = cpu_usage(Some(CpuUsage {
        user: 9_007_199_254_740_993,
        system: 9_007_199_254_740_993,
    }))
    .unwrap();
    assert!(future.user < 0 && future.system < 0);
    for invalid in [-1, i64::MIN] {
        assert!(cpu_usage(Some(CpuUsage {
            user: invalid,
            system: 0
        }))
        .is_err());
        assert!(cpu_usage(Some(CpuUsage {
            user: 0,
            system: invalid
        }))
        .is_err());
    }
}
