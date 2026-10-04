use crate::model::{Account, AccountKind};

pub fn account(name: &str) -> Result<Account, String> {
    let name = name.trim();
    if !(3..=16).contains(&name.len())
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("Offline name must be 3–16 ASCII letters, digits or underscores".into());
    }
    // Minecraft's offline player UUID is the version 3 UUID of the raw MD5
    // digest of "OfflinePlayer:<name>" (without a UUID namespace prefix).
    let mut bytes = md5::compute(format!("OfflinePlayer:{name}")).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Account {
        id: uuid::Uuid::from_bytes(bytes).simple().to_string(),
        name: name.to_owned(),
        kind: AccountKind::Offline,
        xuid: String::new(),
        expires_at: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_identity_is_stable_and_validated() {
        let first = account("Steve").unwrap();
        assert_eq!(first.id, "5627dd98e6be3c21b8a8e92344183641");
        assert_eq!(account("Steve").unwrap().id, first.id);
        assert_ne!(account("steve").unwrap().id, first.id);
        assert!(account("a/b").is_err());
        assert!(account("name with spaces").is_err());
    }
}
