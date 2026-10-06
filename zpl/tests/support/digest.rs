// Capture-provenance SHA-256. RustCrypto selects CPU instructions at runtime
// where available and otherwise uses its optimized portable implementation.
// https://docs.rs/sha2/0.11.0/sha2/#backends
// This helper is included only by examples and tests, not core rendering.
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

pub fn sha256(data: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(data) {
        write!(hex, "{byte:02x}").unwrap();
    }
    hex
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_vectors() {
        // FIPS 180-4 SHA-256 multi-block test vector (one million ASCII a's).
        // https://csrc.nist.gov/projects/cryptographic-standards-and-guidelines/example-values
        assert_eq!(
            sha256(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }
}
