//! Apple Remote Desktop authentication (RFB security type 30) — the native
//! auth of macOS Screen Sharing. Anonymous Diffie-Hellman key agreement, MD5
//! of the shared secret as an AES-128 key, and the username + password sent
//! as a 128-byte AES-ECB-encrypted block. The protocol is unpublished but
//! well understood (gtk-vnc and RealVNC interoperate the same way): the
//! server sends generator, key length, prime, and its public key; the client
//! replies with the encrypted credential block followed by its own public key.

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes128;
use md5::{Digest, Md5};
use num_bigint::BigUint;
use rand::RngCore;

pub struct Response {
    /// AES-128-ECB(username[64] + password[64]), each NUL-terminated with
    /// random padding after the terminator.
    pub ciphertext: [u8; 128],
    /// Our DH public key, big-endian, zero-padded to the prime's length.
    pub client_public: Vec<u8>,
}

/// Compute the type-30 response for the server's DH parameters. `prime` and
/// `server_public` are big-endian as read off the wire; the returned public
/// key has the same length as `prime`.
pub fn auth_response(
    generator: u16,
    prime: &[u8],
    server_public: &[u8],
    username: &str,
    password: &str,
) -> Response {
    let mut rng = rand::thread_rng();
    let mut private = vec![0u8; prime.len()];
    rng.fill_bytes(&mut private);
    // Random padding, not zeros: the credential block must not leak the
    // lengths of the username/password through runs of known plaintext.
    let mut padding = [0u8; 128];
    rng.fill_bytes(&mut padding);
    respond(
        generator,
        prime,
        server_public,
        username,
        password,
        &private,
        padding,
    )
}

/// Deterministic core, split out so tests can pin the private key and padding.
fn respond(
    generator: u16,
    prime: &[u8],
    server_public: &[u8],
    username: &str,
    password: &str,
    private: &[u8],
    mut credentials: [u8; 128],
) -> Response {
    let p = BigUint::from_bytes_be(prime);
    let g = BigUint::from(generator);
    let x = BigUint::from_bytes_be(private);
    let client_public = g.modpow(&x, &p);
    let shared = BigUint::from_bytes_be(server_public).modpow(&x, &p);

    // AES key = MD5 of the shared secret at the prime's full width — leading
    // zero bytes count, so pad before hashing.
    let key = Md5::digest(to_be_padded(&shared, prime.len()));

    let (user_half, pass_half) = credentials.split_at_mut(64);
    write_cstr(user_half, username);
    write_cstr(pass_half, password);

    let cipher = Aes128::new(GenericArray::from_slice(&key));
    for block in credentials.chunks_exact_mut(16) {
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }
    Response {
        ciphertext: credentials,
        client_public: to_be_padded(&client_public, prime.len()),
    }
}

/// NUL-terminated C string at the start of `dst`, truncated on a UTF-8
/// boundary if it can't fit. Bytes after the NUL are left as-is (random).
fn write_cstr(dst: &mut [u8], s: &str) {
    let mut n = s.len().min(dst.len() - 1);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    dst[..n].copy_from_slice(&s.as_bytes()[..n]);
    dst[n] = 0;
}

/// Big-endian bytes of `v`, left-padded with zeros to exactly `len`.
fn to_be_padded(v: &BigUint, len: usize) -> Vec<u8> {
    let bytes = v.to_bytes_be();
    if bytes.len() >= len {
        return bytes[bytes.len() - len..].to_vec();
    }
    let mut out = vec![0u8; len];
    out[len - bytes.len()..].copy_from_slice(&bytes);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockDecrypt;

    /// Textbook DH with p=23, g=5: private 6 gives public 5^6 mod 23 = 8.
    #[test]
    fn dh_public_key_matches_hand_computation() {
        let resp = respond(5, &[23], &[19], "u", "p", &[6], [0u8; 128]);
        assert_eq!(resp.client_public, vec![8]);
    }

    /// Round-trip: decrypting the ciphertext with the key derived from the
    /// same DH math must yield the credential block we intended to send.
    #[test]
    fn credentials_decrypt_with_shared_key() {
        // p = 0xffffffdd (fits in 4 bytes), g = 2. Server private y = 0xabcdef
        // gives server public g^y mod p; we pin our private x and padding.
        let prime = 0xffffffddu32.to_be_bytes();
        let p = BigUint::from(0xffffffddu32);
        let server_priv = BigUint::from(0xabcdefu32);
        let server_pub = BigUint::from(2u32).modpow(&server_priv, &p);
        let server_pub_bytes = {
            let b = server_pub.to_bytes_be();
            let mut out = vec![0u8; 4];
            out[4 - b.len()..].copy_from_slice(&b);
            out
        };
        let private = [0x13, 0x57, 0x9b, 0xdf];
        let mut padding = [0u8; 128];
        for (i, b) in padding.iter_mut().enumerate() {
            *b = i as u8 ^ 0x5a;
        }

        let resp = respond(
            2,
            &prime,
            &server_pub_bytes,
            "magnus",
            "s3cret",
            &private,
            padding,
        );

        // The server would compute shared = client_pub ^ y mod p.
        let client_pub = BigUint::from_bytes_be(&resp.client_public);
        let shared = client_pub.modpow(&server_priv, &p);
        let key = Md5::digest(to_be_padded(&shared, prime.len()));
        let cipher = Aes128::new(GenericArray::from_slice(&key));
        let mut plain = resp.ciphertext;
        for block in plain.chunks_exact_mut(16) {
            cipher.decrypt_block(GenericArray::from_mut_slice(block));
        }

        assert_eq!(&plain[..7], b"magnus\0");
        assert_eq!(&plain[64..71], b"s3cret\0");
        // Random padding preserved after each terminator, not zeroed.
        assert_eq!(plain[7], 7u8 ^ 0x5a);
        assert_eq!(plain[71], 71u8 ^ 0x5a);
    }

    #[test]
    fn cstr_truncates_on_utf8_boundary() {
        let mut buf = [0xffu8; 8];
        write_cstr(&mut buf, "abcdé"); // é is 2 bytes; fits
        assert_eq!(&buf[..7], b"abcd\xc3\xa9\0");
        let mut tight = [0xffu8; 6];
        write_cstr(&mut tight, "abcdé"); // é would straddle the NUL slot
        assert_eq!(&tight[..5], b"abcd\0");
    }

    #[test]
    fn public_key_is_zero_padded_to_prime_width() {
        // Tiny numbers, wide prime buffer: result must still be 8 bytes.
        let prime = [0, 0, 0, 0, 0, 0, 0, 23];
        let resp = respond(
            5,
            &prime,
            &[0, 0, 0, 0, 0, 0, 0, 19],
            "u",
            "p",
            &[6],
            [0u8; 128],
        );
        assert_eq!(resp.client_public, vec![0, 0, 0, 0, 0, 0, 0, 8]);
    }
}
