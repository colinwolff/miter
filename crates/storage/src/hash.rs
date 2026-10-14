//! SHA-256, and the hex rendering of its digest.
//!
//! Hand-written rather than pulled from a crate. The reasons, in order:
//!
//! * **It is the whole of what is needed and it is small.** The ledger keys on a *change detector*,
//!   not on a signature: FNV-1a would do for the job, and SHA-256 is used because it is what a
//!   reviewer recognizes and because a future tool may want to show the digest. Either way the
//!   requirement is one function with one call site, and the workspace’s dependency rules forbid adding a dependency
//!   that the objective did not name.
//! * **The alternative is a dependency for one call site.** `sha2` would be a new crate in the
//!   graph to compute a 32-byte digest of a source file.
//! * **It is testable to the byte.** The NIST vectors in the test module pin the implementation: if
//!   this file ever disagrees with the published digest of `""`, `"abc"` or the two-block
//!   `"abcdbcdecdef…"`, the test says so. A change detector that silently computes something *else*
//!   is a change detector that stops noticing edits.
//!
//! The implementation is the standard one — 512-bit blocks, 64 rounds, constants from FIPS 180-4 —
//! written with `wrapping_*` arithmetic so an overflow is a rotation rather than a panic in debug.

/// The eight round constants: the first 32 bits of the fractional parts of the cube roots of the
/// first sixty-four primes.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The initial hash state: the first 32 bits of the fractional parts of the square roots of the
/// first eight primes.
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// How many bytes a digest occupies.
pub const DIGEST_BYTES: usize = 32;

/// How many hex characters [`sha256_hex`] produces.
pub const HASH_HEX_LEN: usize = 2 * DIGEST_BYTES;

/// The SHA-256 digest of `bytes`.
///
/// A one-shot implementation over a whole slice. The callers here hash a file that has just been
/// read into memory, so a streaming API would be a second code path for one use, and the indexer
/// has to hold the text anyway to parse it.
pub fn sha256(bytes: &[u8]) -> [u8; DIGEST_BYTES] {
    let mut state = H0;
    let mut block = [0u8; 64];
    let mut filled = 0usize;

    for byte in bytes {
        block[filled] = *byte;
        filled += 1;
        if filled == 64 {
            compress(&mut state, &block);
            filled = 0;
        }
    }

    // Padding: a `1` bit, zeros, and the length in bits as a big-endian `u64` in the last eight
    // bytes. One block is enough unless the message ends within eight bytes of a boundary, where
    // the length needs a block of its own.
    let bit_length = (bytes.len() as u64).wrapping_mul(8);
    block[filled] = 0x80;
    filled += 1;
    if filled > 56 {
        for slot in block.iter_mut().skip(filled) {
            *slot = 0;
        }
        compress(&mut state, &block);
        filled = 0;
    }
    for slot in block.iter_mut().skip(filled).take(56 - filled) {
        *slot = 0;
    }
    block[56..64].copy_from_slice(&bit_length.to_be_bytes());
    compress(&mut state, &block);

    let mut digest = [0u8; DIGEST_BYTES];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

/// The lowercase hex rendering of a digest, [`HASH_HEX_LEN`] characters long.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha256(bytes);
    let mut out = String::with_capacity(HASH_HEX_LEN);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// One 512-bit compression: the message schedule, then the sixty-four rounds.
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut schedule = [0u32; 64];
    for (index, word) in schedule.iter_mut().take(16).enumerate() {
        let start = index * 4;
        *word = u32::from_be_bytes([
            block[start],
            block[start + 1],
            block[start + 2],
            block[start + 3],
        ]);
    }
    for index in 16..64 {
        let s0 = schedule[index - 15].rotate_right(7)
            ^ schedule[index - 15].rotate_right(18)
            ^ (schedule[index - 15] >> 3);
        let s1 = schedule[index - 2].rotate_right(17)
            ^ schedule[index - 2].rotate_right(19)
            ^ (schedule[index - 2] >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(s0)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(s1);
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    for index in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choice = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(choice)
            .wrapping_add(K[index])
            .wrapping_add(schedule[index]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(majority);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nist_vectors_agree_to_the_byte() {
        // FIPS 180-4's own examples. These are the *published* digests, not this implementation's
        // output, which is the only way the test can fail when it should.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // The padding boundary: 55 bytes fit one block, 56 need two.
        assert_eq!(
            sha256_hex(&[b'a'; 55]),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 56]),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
    }

    #[test]
    fn the_hex_form_is_two_characters_per_byte() {
        let digest = sha256(b"miter");
        assert_eq!(digest.len(), DIGEST_BYTES);
        let hex = sha256_hex(b"miter");
        assert_eq!(hex.len(), HASH_HEX_LEN);
        assert!(hex.chars().all(|character| character.is_ascii_hexdigit()));
        // A one-byte edit changes the digest, which is the property the ledger depends on.
        assert_ne!(sha256(b"miter"), sha256(b"gsdf"));
    }
}
