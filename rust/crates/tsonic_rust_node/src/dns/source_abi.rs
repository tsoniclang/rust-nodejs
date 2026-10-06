impl LookupAddress {
    pub fn address_value(&self) -> String {
        self.address.clone()
    }
}

pub async fn lookup_async(hostname: &str) -> NodeResult<LookupAddress> {
    lookup(hostname)
}

pub async fn resolve4_async(hostname: &str) -> NodeResult<tsonic_rust_js::JsArray<String>> {
    resolve4(hostname).map(tsonic_rust_js::JsArray::from_dense)
}

pub async fn resolve6_async(hostname: &str) -> NodeResult<tsonic_rust_js::JsArray<String>> {
    resolve6(hostname).map(tsonic_rust_js::JsArray::from_dense)
}

pub async fn reverse_async(address: &str) -> NodeResult<tsonic_rust_js::JsArray<String>> {
    reverse(address).map(tsonic_rust_js::JsArray::from_dense)
}

pub fn lookup_callable<E>(
    root: &crate::background::BackgroundTasks<E>,
    hostname: &str,
    callback: tsonic_rust_runtime::Callable<(Option<NodeError>, String, u8), Result<(), E>>,
) -> NodeResult<()>
where
    E: From<NodeError> + 'static,
{
    let hostname = hostname.to_string();
    root.spawn(
        move || lookup(&hostname),
        move |result| {
            let arguments = match result {
                Ok(result) => (None, result.address, result.family),
                Err(error) => (Some(error), String::new(), 0),
            };
            callback.call(arguments)
        },
    )
}

pub fn resolve4_callable<E>(
    root: &crate::background::BackgroundTasks<E>,
    hostname: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, tsonic_rust_js::JsArray<String>),
        Result<(), E>,
    >,
) -> NodeResult<()>
where
    E: From<NodeError> + 'static,
{
    resolve_addresses_callable(root, hostname, callback, resolve4)
}

pub fn resolve6_callable<E>(
    root: &crate::background::BackgroundTasks<E>,
    hostname: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, tsonic_rust_js::JsArray<String>),
        Result<(), E>,
    >,
) -> NodeResult<()>
where
    E: From<NodeError> + 'static,
{
    resolve_addresses_callable(root, hostname, callback, resolve6)
}

pub fn reverse_callable<E>(
    root: &crate::background::BackgroundTasks<E>,
    address: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, tsonic_rust_js::JsArray<String>),
        Result<(), E>,
    >,
) -> NodeResult<()>
where
    E: From<NodeError> + 'static,
{
    resolve_addresses_callable(root, address, callback, reverse)
}

fn resolve_addresses_callable<E>(
    root: &crate::background::BackgroundTasks<E>,
    input: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, tsonic_rust_js::JsArray<String>),
        Result<(), E>,
    >,
    resolve: fn(&str) -> NodeResult<Vec<String>>,
) -> NodeResult<()>
where
    E: From<NodeError> + 'static,
{
    let input = input.to_string();
    root.spawn(
        move || resolve(&input),
        move |result| {
            let arguments = match result {
                Ok(result) => (None, tsonic_rust_js::JsArray::from_dense(result)),
                Err(error) => (Some(error), tsonic_rust_js::JsArray::new()),
            };
            callback.call(arguments)
        },
    )
}
