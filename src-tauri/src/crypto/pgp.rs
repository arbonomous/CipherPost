use std::io::Read;

use sequoia_openpgp::armor;
use sequoia_openpgp::parse::stream::{
    DecryptionHelper, DecryptorBuilder, DetachedVerifierBuilder, MessageLayer, MessageStructure,
    VerificationHelper,
};
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::{Cert, KeyHandle, Packet, PacketPile};

use crate::error::{CipherError, CipherResult};
use crate::models::PgpStatus;

/// Structured output from MIME & OpenPGP inspection.
#[derive(Debug, Clone)]
pub struct PgpInspection {
    pub status: PgpStatus,
    pub fingerprint: Option<String>,
    pub signer: Option<String>,
}

impl Default for PgpInspection {
    fn default() -> Self {
        Self {
            status: PgpStatus::None,
            fingerprint: None,
            signer: None,
        }
    }
}

/// Inspects MIME content-types, optional detached PGP signature bytes, and inline body text
/// for PGP/MIME (`multipart/signed`, `multipart/encrypted`) and inline OpenPGP armor.
pub fn inspect_pgp(
    content_type_header: &str,
    body_text: &str,
    detached_sig_bytes: Option<&[u8]>,
    signed_payload_bytes: Option<&[u8]>,
    trusted_certs: &[Cert],
) -> PgpInspection {
    let ct_lower = content_type_header.to_ascii_lowercase();

    // 1. PGP/MIME Encrypted (`multipart/encrypted; protocol="application/pgp-encrypted"`)
    if ct_lower.contains("multipart/encrypted")
        || ct_lower.contains("application/pgp-encrypted")
        || body_text.contains("-----BEGIN PGP MESSAGE-----")
    {
        let fp = extract_key_hint_from_armor(body_text.as_bytes());
        return PgpInspection {
            status: PgpStatus::Encrypted,
            fingerprint: fp,
            signer: None,
        };
    }

    // 2. PGP/MIME Signed (`multipart/signed; protocol="application/pgp-signature"`)
    if ct_lower.contains("multipart/signed") || ct_lower.contains("application/pgp-signature") {
        if let (Some(sig), Some(payload)) = (detached_sig_bytes, signed_payload_bytes) {
            if let Ok(verified) = verify_detached_signature(payload, sig, trusted_certs) {
                return verified;
            }
            let fp = extract_key_hint_from_armor(sig);
            return PgpInspection {
                status: PgpStatus::SignedUnverified,
                fingerprint: fp,
                signer: None,
            };
        }

        return PgpInspection {
            status: PgpStatus::SignedUnverified,
            fingerprint: None,
            signer: None,
        };
    }

    // 3. Inline Clearsigned OpenPGP (`-----BEGIN PGP SIGNED MESSAGE-----`)
    if body_text.contains("-----BEGIN PGP SIGNED MESSAGE-----") {
        let fp = extract_key_hint_from_armor(body_text.as_bytes());
        return PgpInspection {
            status: PgpStatus::SignedUnverified,
            fingerprint: fp,
            signer: None,
        };
    }

    PgpInspection::default()
}

/// Parses OpenPGP armored or binary packets via `sequoia-openpgp` to extract issuer fingerprint/KeyID.
fn extract_key_hint_from_armor(bytes: &[u8]) -> Option<String> {
    let mut reader = armor::Reader::from_bytes(bytes, armor::ReaderMode::Tolerant(None));
    let mut decoded = Vec::new();
    let packet_bytes = if reader.read_to_end(&mut decoded).is_ok() && !decoded.is_empty() {
        decoded.as_slice()
    } else {
        bytes
    };

    let pile = PacketPile::from_bytes(packet_bytes).ok()?;
    for packet in pile.descendants() {
        match packet {
            Packet::Signature(sig) => {
                if let Some(fp) = sig.issuer_fingerprints().next() {
                    return Some(fp.to_hex());
                }
                if let Some(kid) = sig.issuers().next() {
                    return Some(kid.to_hex());
                }
            }
            Packet::PKESK(pkesk) => {
                return Some(pkesk.recipient().to_hex());
            }
            _ => {}
        }
    }
    None
}

struct KeyringVerifier<'a> {
    certs: &'a [Cert],
    verified_fp: Option<String>,
    verified_uid: Option<String>,
    good_sig: bool,
}

impl<'a> VerificationHelper for KeyringVerifier<'a> {
    fn get_certs(&mut self, _ids: &[KeyHandle]) -> sequoia_openpgp::Result<Vec<Cert>> {
        Ok(self.certs.to_vec())
    }

    fn check(&mut self, structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        for layer in structure.into_iter() {
            if let MessageLayer::SignatureGroup { results } = layer {
                for result in results {
                    if let Ok(good) = result {
                        self.good_sig = true;
                        self.verified_fp = Some(good.ka.cert().fingerprint().to_hex());
                        self.verified_uid = good
                            .ka
                            .cert()
                            .userids()
                            .next()
                            .map(|u| String::from_utf8_lossy(u.userid().value()).to_string());
                    }
                }
            }
        }
        Ok(())
    }
}

/// Verifies a detached OpenPGP signature (`application/pgp-signature`) against local trusted certificates.
pub fn verify_detached_signature(
    payload: &[u8],
    signature: &[u8],
    trusted_certs: &[Cert],
) -> CipherResult<PgpInspection> {
    if trusted_certs.is_empty() {
        return Ok(PgpInspection {
            status: PgpStatus::SignedUnverified,
            fingerprint: extract_key_hint_from_armor(signature),
            signer: None,
        });
    }

    let policy = StandardPolicy::new();
    let helper = KeyringVerifier {
        certs: trusted_certs,
        verified_fp: None,
        verified_uid: None,
        good_sig: false,
    };

    let mut verifier = DetachedVerifierBuilder::from_bytes(signature)
        .map_err(|e| CipherError::Pgp(e.to_string()))?
        .with_policy(&policy, None, helper)
        .map_err(|e| CipherError::Pgp(e.to_string()))?;

    verifier
        .verify_bytes(payload)
        .map_err(|e| CipherError::Pgp(e.to_string()))?;

    let helper = verifier.into_helper();
    if helper.good_sig {
        Ok(PgpInspection {
            status: PgpStatus::SignedVerified,
            fingerprint: helper.verified_fp,
            signer: helper.verified_uid,
        })
    } else {
        Ok(PgpInspection {
            status: PgpStatus::SignedUnverified,
            fingerprint: extract_key_hint_from_armor(signature),
            signer: None,
        })
    }
}

struct SimpleDecryptHelper {
    secret_cert: Cert,
}

impl VerificationHelper for SimpleDecryptHelper {
    fn get_certs(&mut self, _ids: &[KeyHandle]) -> sequoia_openpgp::Result<Vec<Cert>> {
        Ok(vec![self.secret_cert.clone()])
    }

    fn check(&mut self, _structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        Ok(())
    }
}

impl DecryptionHelper for SimpleDecryptHelper {
    fn decrypt<D>(
        &mut self,
        pkesks: &[sequoia_openpgp::packet::PKESK],
        _skesks: &[sequoia_openpgp::packet::SKESK],
        sym_algo: Option<sequoia_openpgp::types::SymmetricAlgorithm>,
        mut decrypt: D,
    ) -> sequoia_openpgp::Result<Option<sequoia_openpgp::Fingerprint>>
    where
        D: FnMut(
            sequoia_openpgp::types::SymmetricAlgorithm,
            &sequoia_openpgp::crypto::SessionKey,
        ) -> bool,
    {
        let policy = StandardPolicy::new();
        for pkesk in pkesks {
            for ka in self
                .secret_cert
                .keys()
                .unencrypted_secret()
                .with_policy(&policy, None)
                .supported()
                .alive()
                .revoked(false)
                .for_transport_encryption()
            {
                if let Ok(mut keypair) = ka.key().clone().into_keypair() {
                    if let Some((algo, sk)) = pkesk.decrypt(&mut keypair, sym_algo) {
                        if decrypt(algo, &sk) {
                            return Ok(Some(self.secret_cert.fingerprint()));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}

/// Modular hook for decrypting an OpenPGP message using a user's local secret `Cert`.
pub fn decrypt_pgp_message(ciphertext: &[u8], secret_cert: Cert) -> CipherResult<Vec<u8>> {
    let policy = StandardPolicy::new();
    let helper = SimpleDecryptHelper { secret_cert };
    let mut decryptor = DecryptorBuilder::from_bytes(ciphertext)
        .map_err(|e| CipherError::Pgp(e.to_string()))?
        .with_policy(&policy, None, helper)
        .map_err(|e| CipherError::Pgp(e.to_string()))?;

    let mut plaintext = Vec::new();
    decryptor
        .read_to_end(&mut plaintext)
        .map_err(|e| CipherError::Pgp(e.to_string()))?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sequoia_openpgp::cert::CertBuilder;
    use sequoia_openpgp::serialize::stream::{Message, Signer};
    use std::io::Write;

    #[test]
    fn verifies_real_sequoia_detached_pgp_signature() {
        let (cert, _rev) = CertBuilder::general_purpose(None, Some("alice@cipherpost.local"))
            .generate()
            .expect("generate test PGP cert");
        let policy = StandardPolicy::new();

        let signing_keypair = cert
            .keys()
            .unencrypted_secret()
            .with_policy(&policy, None)
            .supported()
            .alive()
            .revoked(false)
            .for_signing()
            .next()
            .expect("signing key")
            .key()
            .clone()
            .into_keypair()
            .expect("keypair");

        let payload = b"Zero-trust verified payload from CipherPost";
        let mut sig_buf = Vec::new();
        {
            let msg = Message::new(&mut sig_buf);
            let mut signer = Signer::new(msg, signing_keypair)
                .detached()
                .build()
                .expect("build detached signer");
            signer.write_all(payload).expect("write payload");
            signer.finalize().expect("finalize signature");
        }

        let inspection = inspect_pgp(
            r#"multipart/signed; protocol="application/pgp-signature""#,
            "",
            Some(&sig_buf),
            Some(payload),
            &[cert.clone()],
        );

        assert_eq!(inspection.status, PgpStatus::SignedVerified);
        assert_eq!(
            inspection.fingerprint.as_deref(),
            Some(cert.fingerprint().to_hex().as_str())
        );
        assert_eq!(
            inspection.signer.as_deref(),
            Some("alice@cipherpost.local")
        );
    }

    #[test]
    fn detects_armored_encrypted_message() {
        let body = "-----BEGIN PGP MESSAGE-----\nVersion: GnuPG\n-----END PGP MESSAGE-----";
        let res = inspect_pgp("text/plain", body, None, None, &[]);
        assert_eq!(res.status, PgpStatus::Encrypted);
    }
}
