use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::signature::{RSA_PKCS1_2048_8192_SHA256, RsaPublicKeyComponents};
use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Deserialize)]
pub struct KeySet {
    keys: Vec<Key>,
}

#[derive(Deserialize)]
struct Key {
    kty: String,
    kid: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: Option<String>,
}

#[derive(Deserialize)]
pub struct AccessToken {
    exp: i64,
    #[serde(default)]
    aud: Audience,
    iss: Option<String>,
    pub email: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
    #[default]
    None,
}

impl AccessToken {
    pub fn parse(token: &str) -> Option<Self> {
        unverified_payload(token)
    }

    pub fn is_live(&self, now: i64) -> bool {
        now <= self.exp
    }

    pub fn is_for(&self, aud: &str) -> bool {
        match &self.aud {
            Audience::One(one) => one == aud,
            Audience::Many(many) => many.iter().any(|item| item == aud),
            Audience::None => false,
        }
    }

    pub fn is_issued_by(&self, auth_domain: &str) -> bool {
        self.iss.as_deref() == Some(format!("https://{auth_domain}").as_str())
    }
}

pub fn unverified_payload<T: DeserializeOwned>(token: &str) -> Option<T> {
    let mut parts = token.split('.');
    let (_, payload, _) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()
}

pub fn verify_rs256<T: DeserializeOwned>(token: &str, keys: &KeySet) -> Result<T, String> {
    let malformed = || "the metadata JWT is malformed".to_string();
    let (signing_input, signature) = token.rsplit_once('.').ok_or_else(malformed)?;
    let (header, payload) = signing_input.split_once('.').ok_or_else(malformed)?;
    let decode = |part: &str| URL_SAFE_NO_PAD.decode(part).map_err(|_| malformed());
    let header: Header = serde_json::from_slice(&decode(header)?).map_err(|_| malformed())?;
    if header.alg != "RS256" {
        return Err(format!("the metadata JWT uses {}, not RS256", header.alg));
    }
    let signature = decode(signature)?;
    let verified = keys
        .keys
        .iter()
        .filter(|key| key.kty == "RSA")
        .filter(|key| header.kid.is_none() || key.kid == header.kid)
        .filter_map(|key| {
            Some((
                decode(key.n.as_deref()?).ok()?,
                decode(key.e.as_deref()?).ok()?,
            ))
        })
        .any(|(n, e)| {
            RsaPublicKeyComponents { n, e }
                .verify(
                    &RSA_PKCS1_2048_8192_SHA256,
                    signing_input.as_bytes(),
                    &signature,
                )
                .is_ok()
        });
    if !verified {
        return Err("the metadata JWT is not signed by the Access team's keys".to_string());
    }
    serde_json::from_slice(&decode(payload)?).map_err(|_| malformed())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const METADATA_JWT: &str = include_str!("testdata/metadata.jwt").trim_ascii();
    pub const METADATA_JWKS: &str = include_str!("testdata/metadata-jwks.json");

    #[derive(Deserialize, Debug, PartialEq)]
    struct Claims {
        hostname: String,
        aud: String,
    }

    fn keys() -> KeySet {
        serde_json::from_str(METADATA_JWKS).unwrap()
    }

    #[test]
    fn accepts_a_jwt_signed_by_a_key_in_the_set() {
        let claims: Claims = verify_rs256(METADATA_JWT, &keys()).unwrap();
        assert_eq!(
            claims,
            Claims {
                hostname: "nestodo.example.com".into(),
                aud: "0123abcd".into()
            }
        );
    }

    #[test]
    fn rejects_claims_that_were_changed_after_signing() {
        let (header, rest) = METADATA_JWT.split_once('.').unwrap();
        let (_, signature) = rest.split_once('.').unwrap();
        let forged = URL_SAFE_NO_PAD.encode(
            r#"{"type":"match","hostname":"evil.example.com","auth_domain":"family.cloudflareaccess.com","aud":"0123abcd","iat":1792000000}"#,
        );
        let tampered = format!("{header}.{forged}.{signature}");
        assert!(verify_rs256::<Claims>(&tampered, &keys()).is_err());
    }

    #[test]
    fn rejects_a_jwt_whose_key_is_not_in_the_set() {
        let other_kid = METADATA_JWKS.replace("test-kid", "rotated-kid");
        let keys: KeySet = serde_json::from_str(&other_kid).unwrap();
        assert!(verify_rs256::<Claims>(METADATA_JWT, &keys).is_err());
    }

    #[test]
    fn rejects_an_unsigned_jwt_even_with_valid_claims() {
        let (_, rest) = METADATA_JWT.split_once('.').unwrap();
        let (payload, _) = rest.split_once('.').unwrap();
        let none = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","kid":"test-kid"}"#);
        assert!(verify_rs256::<Claims>(&format!("{none}.{payload}."), &keys()).is_err());
    }

    pub fn token(payload: &str) -> String {
        format!("e30.{}.c2ln", URL_SAFE_NO_PAD.encode(payload))
    }

    #[test]
    fn an_access_token_is_live_until_its_exp() {
        let parsed = AccessToken::parse(&token(r#"{"exp":100,"aud":"app"}"#)).unwrap();
        assert!(parsed.is_live(100));
        assert!(!parsed.is_live(101));
    }

    #[test]
    fn an_access_token_matches_audiences_given_as_a_string_or_a_list() {
        let one = AccessToken::parse(&token(r#"{"exp":1,"aud":"app"}"#)).unwrap();
        let many = AccessToken::parse(&token(r#"{"exp":1,"aud":["other","app"]}"#)).unwrap();
        let none = AccessToken::parse(&token(r#"{"exp":1}"#)).unwrap();
        assert!(one.is_for("app") && many.is_for("app"));
        assert!(!one.is_for("other-app") && !none.is_for("app"));
    }

    #[test]
    fn values_that_are_not_jwts_are_not_access_tokens() {
        assert!(AccessToken::parse("").is_none());
        assert!(AccessToken::parse("deleted").is_none());
        assert!(AccessToken::parse(&format!("{}.extra", token(r#"{"exp":1}"#))).is_none());
    }
}
