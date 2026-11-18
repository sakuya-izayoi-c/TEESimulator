// Device trait implementations for the software TA.
//
// There is no trusted root-key backend. Operations that require one must fail;
// in particular, do not fall back to an old software root to open a keyblob.

use kmr_common::crypto::{self, mldsa, AccumulatingOperation, Hkdf, MlDsa, OpaqueOr};
use kmr_common::{km_err, rpc_err, Error};
use kmr_ta::device::{
    DiceInfo, RetrieveAttestationIds, RetrieveKeyMaterial, RetrieveRpcArtifacts, RpcV2Req,
};
use kmr_wire::types::AttestationIdInfo;

/// No device root material is provisioned for the software TA.
pub struct Keys;

impl RetrieveKeyMaterial for Keys {
    fn root_kek(&self, _context: &[u8]) -> Result<crypto::OpaqueOr<crypto::hmac::Key>, Error> {
        Err(km_err!(HardwareTypeUnavailable, "no trusted root KEK backend"))
    }
    fn kak(&self) -> Result<crypto::OpaqueOr<crypto::aes::Key>, Error> {
        Err(km_err!(HardwareTypeUnavailable, "no trusted KAK backend"))
    }
    fn unique_id_hbk(&self, _ckdf: &dyn crypto::Ckdf) -> Result<crypto::hmac::Key, Error> {
        Err(km_err!(HardwareTypeUnavailable, "no trusted unique-ID HBK backend"))
    }
}

/// Monotonic clock backed by `CLOCK_BOOTTIME`.
pub struct Clock;

impl crypto::MonotonicClock for Clock {
    fn now(&self) -> crypto::MillisecondsSinceEpoch {
        let mut time = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // Safety: `time` is a valid structure that the call fills in.
        let rc = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut time) };
        if rc < 0 {
            return crypto::MillisecondsSinceEpoch(0);
        }
        crypto::MillisecondsSinceEpoch(((time.tv_sec * 1000) + (time.tv_nsec / 1_000_000)).into())
    }
}

/// Wall-clock time as milliseconds since the Unix epoch, backed by `CLOCK_REALTIME`.
///
/// This is the value keystore2 would stamp into `Tag::CREATION_DATETIME`; we need it
/// because the emulation wrapper around this device's Keymaster 4.0 HAL reports a
/// pre-KeyMint version, so keystore2 never adds the tag itself (see `ops::ensure_creation_datetime`).
pub fn realtime_ms_since_epoch() -> i64 {
    let mut time = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // Safety: `time` is a valid structure that the call fills in.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut time) };
    if rc < 0 {
        return 0;
    }
    (time.tv_sec * 1000) + (time.tv_nsec / 1_000_000)
}

/// No remote-provisioning backend. The default RPC MAC also needs this HBK.
pub struct NoRpc;

impl RetrieveRpcArtifacts for NoRpc {
    fn derive_bytes_from_hbk(
        &self,
        _hkdf: &dyn Hkdf,
        _context: &[u8],
        _output_len: usize,
    ) -> Result<Vec<u8>, Error> {
        Err(rpc_err!(Failed, "no trusted RPC HBK backend"))
    }

    fn get_dice_info(&self, _test_mode: kmr_wire::rpc::TestMode) -> Result<DiceInfo, Error> {
        Err(km_err!(Unimplemented, "remote provisioning is not supported"))
    }

    fn sign_data(
        &self,
        _ec: &dyn crypto::Ec,
        _data: &[u8],
        _rpc_v2: Option<RpcV2Req>,
    ) -> Result<Vec<u8>, Error> {
        Err(km_err!(Unimplemented, "remote provisioning is not supported"))
    }
}

/// Device-ID attestation values for a profile. `get` hands back a clone; the TA
/// vouches for exactly these when an app requests device-ID attestation.
/// `destroy_all` is a no-op — provisioned IDs are never wiped.
pub struct AttestIds(pub AttestationIdInfo);

impl RetrieveAttestationIds for AttestIds {
    fn get(&self) -> Result<AttestationIdInfo, Error> {
        Ok(self.0.clone())
    }

    fn destroy_all(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// Stub ML-DSA backend. The BoringSSL ML-DSA implementation is Soong-only (it uses bssl-sys, absent
/// under Cargo), and TEESimulator never mints post-quantum keys, so this rejects any use cleanly.
/// generate_key/import default to the software seed path in the trait but reach these on first use.
pub struct NoMlDsa;

impl MlDsa for NoMlDsa {
    fn subject_public_key(&self, _key: &OpaqueOr<mldsa::Key>) -> Result<Vec<u8>, Error> {
        Err(km_err!(Unimplemented, "ML-DSA is not supported"))
    }

    fn begin_sign(
        &self,
        _key: OpaqueOr<mldsa::Key>,
    ) -> Result<Box<dyn AccumulatingOperation>, Error> {
        Err(km_err!(Unimplemented, "ML-DSA is not supported"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kmr_common::ErrorKind;
    use kmr_wire::keymint::ErrorCode;

    fn assert_no_root<T>(result: Result<T, Error>) {
        assert!(matches!(
            result.err().as_ref().map(Error::kind),
            Some(ErrorKind::Hal(ErrorCode::HardwareTypeUnavailable, _))
        ));
    }

    #[test]
    fn root_kek_refuses_any_blob_context() {
        let keys: &dyn RetrieveKeyMaterial = &Keys;
        for context in [&[][..], b"stored context"] {
            assert_no_root(keys.root_kek(context));
        }
    }

    #[test]
    fn kak_refuses_shared_secret_negotiation_key() {
        let keys: &dyn RetrieveKeyMaterial = &Keys;
        assert_no_root(keys.kak());
    }

    #[test]
    fn unique_id_refuses_hbk() {
        let keys: &dyn RetrieveKeyMaterial = &Keys;
        assert_no_root(keys.unique_id_hbk(&crypto::NoOpAesCmac));
    }

    // The AOSP blanket HKDF implementation uses HMAC. Neither HKDF nor the
    // default RPC MAC path should reach a cryptographic operation without an HBK.
    struct UnexpectedHmac;

    impl crypto::Hmac for UnexpectedHmac {
        fn begin(
            &self,
            _key: OpaqueOr<crypto::hmac::Key>,
            _digest: kmr_wire::keymint::Digest,
        ) -> Result<Box<dyn AccumulatingOperation>, Error> {
            panic!("RPC used HMAC without a trusted HBK");
        }
    }

    fn assert_no_rpc_hbk(result: Result<Vec<u8>, Error>) {
        assert!(matches!(
            result.err().as_ref().map(Error::kind),
            Some(ErrorKind::Rpc(kmr_wire::rpc::ErrorCode::Failed, _))
        ));
    }

    #[test]
    fn rpc_refuses_hbk_derivation() {
        let rpc: &dyn RetrieveRpcArtifacts = &NoRpc;
        for (context, len) in [(&[][..], 0), (b"context", 32)] {
            assert_no_rpc_hbk(rpc.derive_bytes_from_hbk(&UnexpectedHmac, context, len));
        }
    }

    #[test]
    fn rpc_default_mac_propagates_missing_hbk() {
        let rpc: &dyn RetrieveRpcArtifacts = &NoRpc;
        assert_no_rpc_hbk(rpc.compute_hmac_sha256(&UnexpectedHmac, &UnexpectedHmac, b"input"));
    }
}
