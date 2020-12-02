/// Check if client name and secret are acceptable.
pub fn name_and_secret_acceptable(client: &str, secret: Option<&str>) -> bool {
    !(client.chars().any(|c| !c.is_ascii_alphanumeric())
        || secret
            .unwrap_or("")
            .chars()
            .any(|c| !c.is_ascii_alphanumeric()))
}

/// Build a channel name from a client name and secret.
pub fn channel_name(client: &str, secret: &str) -> String {
    // The log handler should check this.
    assert!(secret.find('_').is_none());
    format!("geohubclient_update_{}_{}", client, secret)
}

/// Extract client name and secret from the database channel name.
pub fn client_secret(channel_name: &str) -> (&str, &str) {
    // Channel name is like geohubclient_update_<client>_<secret>
    let parts = channel_name.split('_').collect::<Vec<&str>>();
    assert!(parts.len() == 4);
    return (parts[2], parts[3]);
}

