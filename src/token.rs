use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use anyhow::Result;
use oauth2::basic::BasicClient;
use oauth2::url::Url;
use oauth2::{reqwest, RefreshToken, RevocationUrl};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};

use crate::OAuthClient;

pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
    auth_client: OAuthClient,
    http_client: reqwest::blocking::Client,
}

impl Token {
    fn fetch_tokens(
        auth_client: OAuthClient,
        http_client: reqwest::blocking::Client,
    ) -> Result<(String, String)> {
        let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

        let (authorize_url, _csrf_state) = auth_client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/calendar.readonly".to_string(),
            ))
            .set_pkce_challenge(pkce_code_challenge)
            .url();

        println!("Open this URL in your browser:\n{authorize_url}\n");

        let (code, _state) = {
            let listener = TcpListener::bind("127.0.0.1:8080").expect("failed to bind listener");

            let Some(mut stream) = listener.incoming().flatten().next() else {
                panic!("listener terminated without accepting a connection");
            };

            let mut reader = BufReader::new(&stream);

            let mut request_line = String::new();
            reader
                .read_line(&mut request_line)
                .expect("failed to read request line");

            let redirect_url = request_line.split_whitespace().nth(1).unwrap();
            let url = Url::parse(&("http://localhost".to_string() + redirect_url)).unwrap();

            let code = url
                .query_pairs()
                .find(|(key, _)| key == "code")
                .map(|(_, code)| AuthorizationCode::new(code.into_owned()))
                .expect("failed to find 'code' in  query string in redirect URL");

            let state = url
                .query_pairs()
                .find(|(key, _)| key == "state")
                .map(|(_, state)| CsrfToken::new(state.into_owned()))
                .expect("failed to find 'state' in query string in redirect URL");

            let message = "Go back to your terminal :)";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
                message.len(),
                message
            );
            stream.write_all(response.as_bytes()).unwrap();

            (code, state)
        };

        let token_response = auth_client
            .exchange_code(code)
            .set_pkce_verifier(pkce_code_verifier)
            .request(&http_client)
            .unwrap();

        Ok((
            token_response.clone().access_token().secret().clone(),
            token_response
                .refresh_token()
                .ok_or_else(|| anyhow::anyhow!("refresh token is not found in response"))?
                .secret()
                .clone(),
        ))
    }
    fn load_tokens() -> Result<(String, String)> {
        // トークンをファイルから読み込む処理を実装
        // 例: JSONファイルから読み込む
        let file = std::fs::File::open("tokens.json")?;
        let tokens: HashMap<String, String> = serde_json::from_reader(file)?;
        Ok((
            tokens.get("access_token").cloned().unwrap_or_default(),
            tokens.get("refresh_token").cloned().unwrap_or_default(),
        ))
    }

    fn save_tokens(access_token: &str, refresh_token: &str) -> Result<()> {
        // トークンをファイルに保存する処理を実装
        let tokens = serde_json::json!({
            "access_token": access_token,
            "refresh_token": refresh_token,
        });
        std::fs::write("tokens.json", tokens.to_string())?;
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        let token_response = self
            .auth_client
            .exchange_refresh_token(&RefreshToken::new(self.refresh_token.clone()))
            .request(&self.http_client)?;

        self.access_token = token_response.access_token().secret().clone();
        Ok(())
    }

    pub fn new(client_id: String, client_secret: String) -> Result<Self> {
        let auth_client = BasicClient::new(ClientId::new(client_id))
            .set_client_secret(ClientSecret::new(client_secret))
            .set_auth_uri(
                AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                    .expect("Invalid authorization endpoint URL"),
            )
            .set_token_uri(
                TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
                    .expect("Invalid token endpoint URL"),
            )
            .set_redirect_uri(
                RedirectUrl::new("http://localhost:8080".to_string())
                    .expect("Invalid redirect URL"),
            )
            .set_revocation_url(
                RevocationUrl::new("https://oauth2.googleapis.com/revoke".to_string())
                    .expect("Invalid revocation endpoint URL"),
            );

        let http_client = reqwest::blocking::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Client should build");

        let token = match Token::load_tokens() {
            Ok((access_token, refresh_token)) => {
                let mut token = Token {
                    access_token,
                    refresh_token,
                    auth_client,
                    http_client,
                };
                token.refresh()?;
                token
            }
            Err(_) => {
                let (access_token, refresh_token) =
                    Token::fetch_tokens(auth_client.clone(), http_client.clone())?;
                Token {
                    access_token,
                    refresh_token,
                    auth_client,
                    http_client,
                }
            }
        };
        Token::save_tokens(token.access_token.as_str(), token.refresh_token.as_str())?;
        Ok(token)
    }
}
