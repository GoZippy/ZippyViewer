//! Downgrade attack prevention.
//!
//! Requirements: 4.1, 4.2, 4.4, 4.5, 4.6

use zrc_proto::v1::{CipherSuiteV1, KexSuiteV1, SigTypeV1};
use crate::error::SecurityError;
use crate::audit::{AuditLogger, SecurityEvent};

/// Minimum required cipher suite version.
pub const MIN_CIPHER_SUITE: CipherSuiteV1 = CipherSuiteV1::HpkeX25519HkdfSha256Chacha20poly1305;

/// Minimum required key exchange suite version.
pub const MIN_KEX_SUITE: KexSuiteV1 = KexSuiteV1::X25519;

/// Minimum required signature type.
pub const MIN_SIG_TYPE: SigTypeV1 = SigTypeV1::Ed25519;

/// Algorithm version checker for downgrade protection.
///
/// Requirements: 4.1, 4.2
pub struct AlgorithmVersionChecker {
    min_cipher_suite: CipherSuiteV1,
    min_kex_suite: KexSuiteV1,
    min_sig_type: SigTypeV1,
}

impl AlgorithmVersionChecker {
    /// Create a new algorithm version checker.
    pub fn new(
        min_cipher_suite: CipherSuiteV1,
        min_kex_suite: KexSuiteV1,
        min_sig_type: SigTypeV1,
    ) -> Self {
        Self {
            min_cipher_suite,
            min_kex_suite,
            min_sig_type,
        }
    }

    /// Check if a cipher suite meets minimum requirements.
    pub fn check_cipher_suite(&self, suite: CipherSuiteV1) -> Result<(), SecurityError> {
        if (suite as i32) < (self.min_cipher_suite as i32) {
            return Err(SecurityError::DowngradeDetected {
                algorithm: format!("cipher_suite:{:?}", suite),
            });
        }
        Ok(())
    }

    /// Check if a key exchange suite meets minimum requirements.
    pub fn check_kex_suite(&self, suite: KexSuiteV1) -> Result<(), SecurityError> {
        if (suite as i32) < (self.min_kex_suite as i32) {
            return Err(SecurityError::DowngradeDetected {
                algorithm: format!("kex_suite:{:?}", suite),
            });
        }
        Ok(())
    }

    /// Check if a signature type meets minimum requirements.
    pub fn check_sig_type(&self, sig_type: SigTypeV1) -> Result<(), SecurityError> {
        if (sig_type as i32) < (self.min_sig_type as i32) {
            return Err(SecurityError::DowngradeDetected {
                algorithm: format!("sig_type:{:?}", sig_type),
            });
        }
        Ok(())
    }
}

impl Default for AlgorithmVersionChecker {
    fn default() -> Self {
        Self::new(MIN_CIPHER_SUITE, MIN_KEX_SUITE, MIN_SIG_TYPE)
    }
}

/// Handshake algorithm verification.
///
/// Verifies that algorithm identifiers in handshake messages are consistent
/// and meet minimum requirements.
///
/// Requirements: 4.4, 4.5
pub struct HandshakeAlgorithmVerifier {
    checker: AlgorithmVersionChecker,
    agreed_cipher_suite: Option<CipherSuiteV1>,
    agreed_kex_suite: Option<KexSuiteV1>,
    agreed_sig_type: Option<SigTypeV1>,
}

impl HandshakeAlgorithmVerifier {
    /// Create a new handshake algorithm verifier.
    pub fn new() -> Self {
        Self {
            checker: AlgorithmVersionChecker::default(),
            agreed_cipher_suite: None,
            agreed_kex_suite: None,
            agreed_sig_type: None,
        }
    }

    /// Record and verify initial algorithm proposal.
    pub fn record_proposal(
        &mut self,
        cipher_suite: CipherSuiteV1,
        kex_suite: KexSuiteV1,
        sig_type: SigTypeV1,
    ) -> Result<(), SecurityError> {
        // Check minimum requirements
        self.checker.check_cipher_suite(cipher_suite)?;
        self.checker.check_kex_suite(kex_suite)?;
        self.checker.check_sig_type(sig_type)?;

        // Record as agreed
        self.agreed_cipher_suite = Some(cipher_suite);
        self.agreed_kex_suite = Some(kex_suite);
        self.agreed_sig_type = Some(sig_type);

        Ok(())
    }

    /// Verify that subsequent handshake messages use the same algorithms.
    pub fn verify_consistency(
        &self,
        cipher_suite: CipherSuiteV1,
        kex_suite: KexSuiteV1,
        sig_type: SigTypeV1,
    ) -> Result<(), SecurityError> {
        if let Some(agreed) = self.agreed_cipher_suite {
            if cipher_suite != agreed {
                return Err(SecurityError::DowngradeDetected {
                    algorithm: format!("cipher_suite changed from {:?} to {:?}", agreed, cipher_suite),
                });
            }
        }

        if let Some(agreed) = self.agreed_kex_suite {
            if kex_suite != agreed {
                return Err(SecurityError::DowngradeDetected {
                    algorithm: format!("kex_suite changed from {:?} to {:?}", agreed, kex_suite),
                });
            }
        }

        if let Some(agreed) = self.agreed_sig_type {
            if sig_type != agreed {
                return Err(SecurityError::DowngradeDetected {
                    algorithm: format!("sig_type changed from {:?} to {:?}", agreed, sig_type),
                });
            }
        }

        Ok(())
    }

    /// Get agreed algorithms.
    pub fn agreed_algorithms(&self) -> Option<(CipherSuiteV1, KexSuiteV1, SigTypeV1)> {
        match (self.agreed_cipher_suite, self.agreed_kex_suite, self.agreed_sig_type) {
            (Some(c), Some(k), Some(s)) => Some((c, k, s)),
            _ => None,
        }
    }
}

impl Default for HandshakeAlgorithmVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Log downgrade detection events.
///
/// Requirements: 4.6
pub fn log_downgrade_detection(
    logger: &AuditLogger,
    algorithm: &str,
    peer_id: Option<&str>,
) -> Result<(), SecurityError> {
    let event = SecurityEvent::IdentityMismatch {
        peer_id: format!("downgrade:{}:{}", algorithm, peer_id.unwrap_or("unknown")),
    };
    logger.log(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_algorithm_version_checker_rejects_weak() {
        let checker = AlgorithmVersionChecker::default();

        // Should reject unspecified/weak algorithms
        assert!(checker.check_cipher_suite(CipherSuiteV1::Unspecified).is_err());
        assert!(checker.check_kex_suite(KexSuiteV1::Unspecified).is_err());
        assert!(checker.check_sig_type(SigTypeV1::Unspecified).is_err());
    }

    #[test]
    fn test_algorithm_version_checker_accepts_strong() {
        let checker = AlgorithmVersionChecker::default();

        // Should accept minimum required algorithms
        assert!(checker.check_cipher_suite(MIN_CIPHER_SUITE).is_ok());
        assert!(checker.check_kex_suite(MIN_KEX_SUITE).is_ok());
        assert!(checker.check_sig_type(MIN_SIG_TYPE).is_ok());
    }

    #[test]
    fn test_handshake_verifier_consistency() {
        let mut verifier = HandshakeAlgorithmVerifier::new();

        // Record initial proposal
        verifier.record_proposal(MIN_CIPHER_SUITE, MIN_KEX_SUITE, MIN_SIG_TYPE).unwrap();

        // Should accept same algorithms
        assert!(verifier.verify_consistency(MIN_CIPHER_SUITE, MIN_KEX_SUITE, MIN_SIG_TYPE).is_ok());

        // Should reject different algorithms
        assert!(verifier.verify_consistency(
            CipherSuiteV1::HpkeX25519HkdfSha256Aesgcm128,
            MIN_KEX_SUITE,
            MIN_SIG_TYPE,
        ).is_err());
    }
}
