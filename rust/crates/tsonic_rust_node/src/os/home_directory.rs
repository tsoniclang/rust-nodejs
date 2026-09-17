#[cfg(unix)]
fn account_home_directory() -> NodeResult<String> {
    let account = nix::unistd::User::from_uid(nix::unistd::Uid::effective())
        .map_err(|error| NodeError::new("ERR_SYSTEM_ERROR", error.to_string()))?
        .ok_or_else(|| {
            NodeError::new(
                "ERR_SYSTEM_ERROR",
                "The effective user has no account directory",
            )
        })?;
    Ok(account.dir.to_string_lossy().into_owned())
}

#[cfg(windows)]
fn account_home_directory() -> NodeResult<String> {
    std::env::home_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or_else(|| {
            NodeError::new(
                "ERR_SYSTEM_ERROR",
                "The current user profile directory is unavailable",
            )
        })
}

#[cfg(not(any(unix, windows)))]
fn account_home_directory() -> NodeResult<String> {
    Err(NodeError::new(
        "ERR_SYSTEM_ERROR",
        "This platform has no supported account directory lookup",
    ))
}
