#[must_use] 
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

use tfhe::prelude::*;
use tfhe::{ConfigBuilder, generate_keys, set_server_key, FheUint64};

/// A proof-of-concept for encrypted token summation.
/// This demonstrates how usage metrics can be aggregated without decrypting individual values.
#[must_use] 
pub fn aggregate_tokens(tokens_a: u64, tokens_b: u64) -> u64 {
    let config = ConfigBuilder::default().build();
    let (client_key, server_key) = generate_keys(config);
    
    // In a real distributed system, the server key would be sent to the aggregator (Redis/Coordinator)
    set_server_key(server_key);
    
    // Client encrypts their usage data
    let encrypted_a = FheUint64::encrypt(tokens_a, &client_key);
    let encrypted_b = FheUint64::encrypt(tokens_b, &client_key);
    
    // Aggregator (server) performs the addition without decryption
    let encrypted_sum = encrypted_a + encrypted_b;
    
    // Client decrypts the final result
    encrypted_sum.decrypt(&client_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypted_summation() {
        let a = 1200;
        let b = 800;
        let result = aggregate_tokens(a, b);
        assert_eq!(result, 2000);
    }
}
