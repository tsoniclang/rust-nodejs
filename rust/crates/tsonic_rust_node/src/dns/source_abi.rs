impl LookupAddress {
    pub fn address_value(&self) -> String {
        self.address.clone()
    }

    pub fn family_number(&self) -> f64 {
        self.family as f64
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
    hostname: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, String, f64),
        Result<(), E>,
    >,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    let hostname = hostname.to_string();
    crate::background::spawn(
        move || lookup(&hostname),
        move |result| {
        let arguments = match result {
            Ok(result) => (None, result.address, result.family as f64),
            Err(error) => (Some(error), String::new(), 0.0),
        };
        callback
            .call(arguments)
            .map_err(crate::error::callback_runtime_error)
    })
}

pub fn resolve4_callable<E>(
    hostname: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, tsonic_rust_js::JsArray<String>),
        Result<(), E>,
    >,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    resolve_addresses_callable(hostname, callback, resolve4)
}

pub fn resolve6_callable<E>(
    hostname: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, tsonic_rust_js::JsArray<String>),
        Result<(), E>,
    >,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    resolve_addresses_callable(hostname, callback, resolve6)
}

pub fn reverse_callable<E>(
    address: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, tsonic_rust_js::JsArray<String>),
        Result<(), E>,
    >,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    resolve_addresses_callable(address, callback, reverse)
}

fn resolve_addresses_callable<E>(
    input: &str,
    callback: tsonic_rust_runtime::Callable<
        (Option<NodeError>, tsonic_rust_js::JsArray<String>),
        Result<(), E>,
    >,
    resolve: fn(&str) -> NodeResult<Vec<String>>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    let input = input.to_string();
    crate::background::spawn(
        move || resolve(&input),
        move |result| {
        let arguments = match result {
            Ok(result) => (None, tsonic_rust_js::JsArray::from_dense(result)),
            Err(error) => (Some(error), tsonic_rust_js::JsArray::new()),
        };
        callback
            .call(arguments)
            .map_err(crate::error::callback_runtime_error)
    })
}
