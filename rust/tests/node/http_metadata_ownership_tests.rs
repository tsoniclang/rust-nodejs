use std::hint::black_box;

use tsonic_rust_node::http::IncomingMessage;
use tsonic_rust_node::NodeError;

#[path = "../../../../rust-runtime/tests/helpers/allocation_counts.rs"]
mod allocation_counts;

#[test]
fn repeated_incoming_metadata_queries_allocate_nothing_and_borrow_the_same_data() {
    let readable_tasks = tsonic_rust_node::runtime_tasks::RuntimeTasks::new();
    let message = IncomingMessage::<NodeError>::new(
        readable_tasks.handle(),
        "POST",
        "/native-resource",
        Vec::new(),
    );
    let alias = message.clone();
    let method = message.method().unwrap();
    let url = message.url().unwrap();
    let version = message.http_version();
    let headers = message.headers();
    let cost = allocation_counts::measure_requests(|| {
        for _ in 0..10_000 {
            assert!(std::ptr::eq(
                method.as_ptr(),
                black_box(alias.method().unwrap()).as_ptr()
            ));
            assert!(std::ptr::eq(
                url.as_ptr(),
                black_box(alias.url().unwrap()).as_ptr()
            ));
            assert!(std::ptr::eq(
                version.as_ptr(),
                black_box(alias.http_version()).as_ptr()
            ));
            assert!(std::ptr::eq(headers, black_box(alias.headers())));
            assert!(std::ptr::eq(headers, black_box(alias.headers_distinct())));
            assert_eq!(black_box(alias.status_code()), None);
            assert_eq!(black_box(alias.status_message()), None);
        }
    });
    assert_eq!(cost, (0, 0));
}
