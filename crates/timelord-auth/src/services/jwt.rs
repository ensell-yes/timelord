use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use uuid::Uuid;

use crate::config::Config;
use timelord_common::{auth_claims::Claims, error::AppError};

pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    pub key_id: String,
    pub access_ttl_secs: i64,
    pub refresh_ttl_secs: i64,
    /// Public key in PEM format — exposed via JWKS endpoint
    pub public_key_pem: String,
}

impl JwtService {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let private_pem = config.jwt_private_key_pem.replace("\\n", "\n");
        let public_pem = config.jwt_public_key_pem.replace("\\n", "\n");

        let encoding_key = EncodingKey::from_rsa_pem(private_pem.as_bytes())
            .map_err(|e| anyhow::anyhow!("Invalid JWT_PRIVATE_KEY_PEM: {e}"))?;
        let decoding_key = DecodingKey::from_rsa_pem(public_pem.as_bytes())
            .map_err(|e| anyhow::anyhow!("Invalid JWT_PUBLIC_KEY_PEM: {e}"))?;

        Ok(Self {
            encoding_key,
            decoding_key,
            key_id: config.jwt_key_id.clone(),
            access_ttl_secs: config.jwt_expiry_seconds,
            refresh_ttl_secs: config.refresh_expiry_seconds,
            public_key_pem: public_pem,
        })
    }

    pub fn encode_access(
        &self,
        user_id: Uuid,
        org_id: Uuid,
        role: &str,
    ) -> Result<String, AppError> {
        let claims = Claims::new(user_id, org_id, role, self.access_ttl_secs);
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.key_id.clone());

        encode(&header, &claims, &self.encoding_key)
            .map_err(|e| AppError::internal(format!("JWT encode: {e}")))
    }

    pub fn decode_access(&self, token: &str) -> Result<TokenData<Claims>, AppError> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;

        decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AppError::Unauthorized,
            _ => AppError::Unauthorized,
        })
    }

    /// Build a JWKS response body (JSON string).
    pub fn jwks_json(&self) -> serde_json::Value {
        // For RS256, we need n and e from the public key.
        // We use a simple representation that indicates the key type.
        // In production, you'd use the rsa crate to extract n/e from the PEM.
        serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": self.key_id,
                // Note: In a real implementation, parse the PEM to extract n and e.
                // For now, clients should use the /auth/public-key endpoint for PEM format.
                "x5c": []
            }]
        })
    }
}

/// Opaque refresh token stored as SHA-256 hash in DB.
pub fn hash_token(token: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    // Use SHA-256 in production; this uses a simple hash for the stub
    // TODO: replace with sha2::Sha256
    let mut hasher = DefaultHasher::new();
    token.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Generate a cryptographically random refresh token.
pub fn generate_refresh_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    use base64::{engine::general_purpose, Engine as _};
    general_purpose::STANDARD.encode(bytes)
}
