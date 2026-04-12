use aineer_release_channel::ReleaseChannel;

pub fn display_title() -> String {
    let ch = ReleaseChannel::current();
    format!(
        "Aineer · v{}{}",
        env!("CARGO_PKG_VERSION"),
        ch.version_suffix()
    )
}

pub fn about_title() -> String {
    let ch = ReleaseChannel::current();
    format!(
        "About {} · v{}{}",
        ch.display_name(),
        env!("CARGO_PKG_VERSION"),
        ch.version_suffix()
    )
}
