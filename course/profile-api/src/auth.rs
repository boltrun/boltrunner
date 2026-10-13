use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Claims {
    pub sub: String,
    pub exp: u64,
    pub iat: u64,
    pub iss: String,
    pub aud: String,
}
#[derive(Clone)]
pub struct Tokens {
    secret: Vec<u8>,
}
impl Tokens {
    pub fn new(secret: &[u8]) -> Result<Self, &'static str> {
        if secret.len() < 32 {
            return Err("JWT secret must contain at least 32 bytes");
        }
        Ok(Self {
            secret: secret.to_vec(),
        })
    }
    pub fn issue(&self, id: u64, now: u64) -> Result<String, jsonwebtoken::errors::Error> {
        self.issue_claims(Claims {
            sub: id.to_string(),
            exp: now + 900,
            iat: now,
            iss: "boltrunner-profile-api".into(),
            aud: "boltrunner-profile-ui".into(),
        })
    }
    pub fn issue_claims(&self, claims: Claims) -> Result<String, jsonwebtoken::errors::Error> {
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(&self.secret),
        )
    }
    pub fn verify(&self, token: &str) -> Result<u64, jsonwebtoken::errors::Error> {
        let mut v = Validation::new(Algorithm::HS256);
        v.leeway = 0;
        v.set_issuer(&["boltrunner-profile-api"]);
        v.set_audience(&["boltrunner-profile-ui"]);
        v.set_required_spec_claims(&["sub", "exp", "iat", "iss", "aud"]);
        let data = decode::<Claims>(token, &DecodingKey::from_secret(&self.secret), &v)?;
        let id = data.claims.sub.parse::<u64>().map_err(|_| {
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidSubject)
        })?;
        if id == 0 {
            return Err(jsonwebtoken::errors::ErrorKind::InvalidSubject.into());
        }
        Ok(id)
    }
}
