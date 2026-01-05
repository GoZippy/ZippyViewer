//! Utility functions for cryptographic operations.
//!
//! Provides constant-time comparison utilities to prevent timing attacks
//! when comparing sensitive values like signatures, MACs, and secrets.
//!
//! Requirements: 11.4, 11.5

use constant_time_eq::constant_time_eq;

/// Compare two byte slices in constant time.
///
/// This function performs a constant-time comparison to prevent timing attacks.
/// It returns `true` if the slices are equal, `false` otherwise.
///
/// # Arguments
///
/// * `a` - First byte slice to compare
/// * `b` - Second byte slice to compare
///
/// # Returns
///
/// `true` if the slices are equal, `false` otherwise.
///
/// # Requirements
///
/// - Requirement 11.4: Use constant-time comparison for all signature and MAC verification
/// - Requirement 11.5: Do not leak timing information about the comparison result
///
/// # Example
///
/// ```rust
/// use zrc_crypto::utils::constant_time_compare;
///
/// let sig1 = [0u8; 64];
/// let sig2 = [0u8; 64];
/// assert!(constant_time_compare(&sig1, &sig2));
///
/// let sig3 = [1u8; 64];
/// assert!(!constant_time_compare(&sig1, &sig3));
/// ```
pub fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
    constant_time_eq(a, b)
}

/// Compare two fixed-size arrays in constant time.
///
/// This is a convenience function for comparing fixed-size arrays.
/// It performs a constant-time comparison to prevent timing attacks.
///
/// # Arguments
///
/// * `a` - First array to compare
/// * `b` - Second array to compare
///
/// # Returns
///
/// `true` if the arrays are equal, `false` otherwise.
///
/// # Requirements
///
/// - Requirement 11.4: Use constant-time comparison for all signature and MAC verification
/// - Requirement 11.5: Do not leak timing information about the comparison result
///
/// # Example
///
/// ```rust
/// use zrc_crypto::utils::constant_time_compare_array;
///
/// let key1 = [0u8; 32];
/// let key2 = [0u8; 32];
/// assert!(constant_time_compare_array(&key1, &key2));
///
/// let key3 = [1u8; 32];
/// assert!(!constant_time_compare_array(&key1, &key3));
/// ```
pub fn constant_time_compare_array<const N: usize>(a: &[u8; N], b: &[u8; N]) -> bool {
    constant_time_eq(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_compare_equal() {
        let a = b"hello world";
        let b = b"hello world";
        assert!(constant_time_compare(a, b));
    }

    #[test]
    fn test_constant_time_compare_different() {
        let a = b"hello world";
        let b = b"hello worlD";
        assert!(!constant_time_compare(a, b));
    }

    #[test]
    fn test_constant_time_compare_different_length() {
        let a = b"hello";
        let b = b"hello world";
        assert!(!constant_time_compare(a, b));
    }

    #[test]
    fn test_constant_time_compare_array_equal() {
        let a = [0u8; 32];
        let b = [0u8; 32];
        assert!(constant_time_compare_array(&a, &b));
    }

    #[test]
    fn test_constant_time_compare_array_different() {
        let a = [0u8; 32];
        let mut b = [0u8; 32];
        b[0] = 1;
        assert!(!constant_time_compare_array(&a, &b));
    }

    #[test]
    fn test_constant_time_compare_signature_sized() {
        let sig1 = [0u8; 64];
        let sig2 = [0u8; 64];
        assert!(constant_time_compare_array(&sig1, &sig2));

        let mut sig3 = [0u8; 64];
        sig3[0] = 1;
        assert!(!constant_time_compare_array(&sig1, &sig3));
    }
}
