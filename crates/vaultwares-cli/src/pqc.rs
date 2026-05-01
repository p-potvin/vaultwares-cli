use fips203::ml_kem_768;
use fips203::traits::{KeyGen, Encaps, Decaps, SerDes};
use std::io;

/// A Post-Quantum Cryptography Key Pair using ML-KEM-768.
pub struct PqcKeyPair {
    pub ek: ml_kem_768::EncapsKey,
    pub dk: ml_kem_768::DecapsKey,
}

impl PqcKeyPair {
    /// Generates a new ML-KEM-768 key pair.
    pub fn generate() -> Self {
        let (ek, dk) = ml_kem_768::KG::try_keygen().expect("PQC KeyGen failed");
        Self { ek, dk }
    }
}

/// Result of a Key Encapsulation operation.
pub struct KEMResult {
    /// The ciphertext to be sent to the receiver.
    pub ciphertext: Vec<u8>,
    /// The shared secret established between parties.
    pub shared_secret: Vec<u8>,
}

/// Encapsulates a shared secret using the provided public key (EncapsKey).
pub fn encapsulate(ek: &ml_kem_768::EncapsKey) -> io::Result<KEMResult> {
    let (ct, ss) = ek.try_encaps().map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("PQC Encapsulation failed: {:?}", e))
    })?;
    
    Ok(KEMResult {
        ciphertext: ct.into_bytes().to_vec(),
        shared_secret: ss.into_bytes().to_vec(),
    })
}

/// Decapsulates a shared secret from the ciphertext using the private key (DecapsKey).
pub fn decapsulate(dk: &ml_kem_768::DecapsKey, ct_bytes: &[u8]) -> io::Result<Vec<u8>> {
    let ct_array: [u8; 1088] = ct_bytes.try_into().map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "Invalid PQC ciphertext length")
    })?;
    let ct = ml_kem_768::CipherText::try_from_bytes(ct_array).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Failed to parse PQC ciphertext: {:?}", e))
    })?;
    
    let ss = dk.try_decaps(&ct).map_err(|e| {
        io::Error::new(io::ErrorKind::Other, format!("PQC Decapsulation failed: {:?}", e))
    })?;
    
    Ok(ss.into_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kem_roundtrip() {
        let keys = PqcKeyPair::generate();
        let kem = encapsulate(&keys.ek).expect("Encapsulation failed");
        let recovered_ss = decapsulate(&keys.dk, &kem.ciphertext).expect("Decapsulation failed");
        
        assert_eq!(kem.shared_secret, recovered_ss);
    }
}

