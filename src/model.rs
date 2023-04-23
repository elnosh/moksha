use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::io::{self};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindedMessage {
    pub amount: u64,
    #[serde(rename = "B_")]
    pub b_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindedSignature {
    pub amount: u64,
    #[serde(rename = "C_")]
    pub c_: String,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    pub amount: u64,
    pub secret: String,
    #[serde(rename = "C")]
    pub c: String,
    pub id: Option<String>,
    pub script: Option<P2SHScript>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2SHScript {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    mint: Option<String>,
    proofs: Proofs,
}

const TOKEN_PREFIX_V3: &str = "cashuA";

impl Token {
    pub fn serialize(&self) -> io::Result<String> {
        let json = serde_json::to_string(&self)?;
        Ok(format!(
            "{}{}",
            TOKEN_PREFIX_V3,
            general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes())
        ))
    }

    pub fn deserialize(data: String) -> io::Result<Token> {
        if !data.starts_with(TOKEN_PREFIX_V3) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid token prefix",
            ));
        }

        let json = general_purpose::URL_SAFE_NO_PAD
            .decode(
                data.strip_prefix(TOKEN_PREFIX_V3)
                    .expect("Token does not contain prefix")
                    .as_bytes(),
            )
            .unwrap(); // FIXME: handle error
        let token = serde_json::from_slice::<Token>(&json)?;
        Ok(token)
    }
}

pub type Proofs = Vec<Proof>;
pub type Tokens = Vec<Token>;

#[cfg(test)]
mod tests {
    use crate::model::{Proof, Token};
    use serde_json::json;

    #[test]
    fn test_proof() -> anyhow::Result<()> {
        let js = json!(
            {
              "id": "DSAl9nvvyfva",
              "amount": 2,
              "secret": "EhpennC9qB3iFlW8FZ_pZw",
              "C": "02c020067db727d586bc3183aecf97fcb800c3f4cc4759f69c626c9db5d8f5b5d4"
            }
        );

        let proof = serde_json::from_value::<Proof>(js)?;
        assert_eq!(proof.amount, 2);
        assert_eq!(proof.id, Some("DSAl9nvvyfva".to_string()));
        assert_eq!(proof.secret, "EhpennC9qB3iFlW8FZ_pZw".to_string());
        assert_eq!(
            proof.c,
            "02c020067db727d586bc3183aecf97fcb800c3f4cc4759f69c626c9db5d8f5b5d4".to_string()
        );
        Ok(())
    }

    #[test]
    fn test_token() -> anyhow::Result<()> {
        let js = json!(
            {
              "mint": "https://8333.space:3338",
              "proofs": [
                {
                  "id": "DSAl9nvvyfva",
                  "amount": 2,
                  "secret": "EhpennC9qB3iFlW8FZ_pZw",
                  "C": "02c020067db727d586bc3183aecf97fcb800c3f4cc4759f69c626c9db5d8f5b5d4"
                },
                {
                  "id": "DSAl9nvvyfva",
                  "amount": 8,
                  "secret": "TmS6Cv0YT5PU_5ATVKnukw",
                  "C": "02ac910bef28cbe5d7325415d5c263026f15f9b967a079ca9779ab6e5c2db133a7"
                }
              ]
        });

        let token = serde_json::from_value::<super::Token>(js)?;
        assert_eq!(token.mint, Some("https://8333.space:3338".to_string()));
        assert_eq!(token.proofs.len(), 2);
        Ok(())
    }

    #[test]
    fn test_token_serialize() -> anyhow::Result<()> {
        let token = Token {
            mint: Some("mymint".to_string()),
            proofs: vec![Proof {
                amount: 21,
                secret: "secret".to_string(),
                c: "c".to_string(),
                id: None,
                script: None,
            }],
        };

        let serialized = token.serialize()?;
        println!("{}", serialized);
        Ok(())
    }

    #[test]
    fn test_token_deserialize() -> anyhow::Result<()> {
        let input = "cashuAeyJtaW50IjoibXltaW50IiwicHJvb2ZzIjpbeyJhbW91bnQiOjIxLCJzZWNyZXQiOiJzZWNyZXQiLCJDIjoiYyIsImlkIjpudWxsLCJzY3JpcHQiOm51bGx9XX0";
        let token = Token::deserialize(input.to_string())?;
        assert_eq!(token.mint, Some("mymint".to_string()),);
        assert_eq!(token.proofs.len(), 1);
        Ok(())
    }
}
