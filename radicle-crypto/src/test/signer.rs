use crate::{KeyPair, PublicKey, SecretKey, Seed, Signature, Signer, SignerError};

#[derive(Debug, Clone)]
pub struct MockSigner {
    pk: PublicKey,
    sk: SecretKey,
}

impl MockSigner {
    pub fn new(rng: &mut fastrand::Rng) -> Self {
        let mut bytes: [u8; 32] = [0; 32];

        for byte in &mut bytes {
            *byte = rng.u8(..);
        }
        let seed = Seed::new(bytes);
        let keypair = KeyPair::from_seed(seed);

        Self::from(keypair.sk)
    }
}

impl From<SecretKey> for MockSigner {
    fn from(sk: SecretKey) -> Self {
        let pk = sk.public_key().into();
        Self { sk, pk }
    }
}

impl Default for MockSigner {
    fn default() -> Self {
        let seed = Seed::generate();
        let keypair = KeyPair::from_seed(seed);
        let sk = keypair.sk;

        Self {
            pk: sk.public_key().into(),
            sk,
        }
    }
}

impl PartialEq for MockSigner {
    fn eq(&self, other: &Self) -> bool {
        self.pk == other.pk
    }
}

impl Eq for MockSigner {}

impl std::hash::Hash for MockSigner {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pk.hash(state)
    }
}

impl Signer for MockSigner {
    fn public_key(&self) -> &PublicKey {
        &self.pk
    }

    fn sign(&self, msg: &[u8]) -> Signature {
        self.sk.sign(msg, None).into()
    }

    fn try_sign(&self, msg: &[u8]) -> Result<Signature, SignerError> {
        Ok(self.sign(msg))
    }
}
