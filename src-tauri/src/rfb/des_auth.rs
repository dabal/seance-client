//! VNC authentication (security type 2): DES-encrypt the 16-byte server
//! challenge with the password as key. VNC quirk: each key byte is
//! bit-reversed before use, and the password is truncated/zero-padded to 8
//! bytes — this is why VNC passwords are effectively 8 characters max.

use des::cipher::generic_array::GenericArray;
use des::cipher::{BlockEncrypt, KeyInit};
use des::Des;

pub fn encrypt_challenge(password: &str, challenge: &[u8; 16]) -> [u8; 16] {
    let mut key = [0u8; 8];
    for (i, b) in password.bytes().take(8).enumerate() {
        key[i] = b.reverse_bits();
    }
    let cipher = Des::new(GenericArray::from_slice(&key));
    let mut out = *challenge;
    for chunk in out.chunks_exact_mut(8) {
        cipher.encrypt_block(GenericArray::from_mut_slice(chunk));
    }
    out
}
