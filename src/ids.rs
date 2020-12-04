/// Check if client name and secret are acceptable.
pub fn name_and_secret_acceptable(client: &str, secret: Option<&str>) -> bool {
    !(client.chars().any(|c| !c.is_ascii_alphanumeric())
        || secret
            .unwrap_or("")
            .chars()
            .any(|c| !c.is_ascii_alphanumeric()))
}
