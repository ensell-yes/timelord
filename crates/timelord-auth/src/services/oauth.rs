use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenUrl,
};
use reqwest::Client;

use crate::config::Config;
use crate::models::user::{GoogleUserInfo, MicrosoftUserInfo};
use timelord_common::error::AppError;

pub struct OAuthClients {
    pub google: BasicClient,
    pub microsoft: BasicClient,
    pub http: Client,
}

impl OAuthClients {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let google = BasicClient::new(
            ClientId::new(config.google_client_id.clone()),
            Some(ClientSecret::new(config.google_client_secret.clone())),
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                .map_err(|e| anyhow::anyhow!("Google auth URL: {e}"))?,
            Some(
                TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                    .map_err(|e| anyhow::anyhow!("Google token URL: {e}"))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.google_redirect_uri.clone())
                .map_err(|e| anyhow::anyhow!("Google redirect URI: {e}"))?,
        );

        let tenant = &config.microsoft_tenant_id;
        let microsoft = BasicClient::new(
            ClientId::new(config.microsoft_client_id.clone()),
            Some(ClientSecret::new(config.microsoft_client_secret.clone())),
            AuthUrl::new(format!(
                "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"
            ))
            .map_err(|e| anyhow::anyhow!("MS auth URL: {e}"))?,
            Some(
                TokenUrl::new(format!(
                    "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"
                ))
                .map_err(|e| anyhow::anyhow!("MS token URL: {e}"))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.microsoft_redirect_uri.clone())
                .map_err(|e| anyhow::anyhow!("MS redirect URI: {e}"))?,
        );

        Ok(Self {
            google,
            microsoft,
            http: Client::new(),
        })
    }

    /// Build Google authorization URL with PKCE.
    pub fn google_auth_url(&self) -> (String, CsrfToken, PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state) = self
            .google
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/calendar".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/calendar.events".to_string(),
            ))
            .set_pkce_challenge(pkce_challenge)
            .url();
        (url.to_string(), state, pkce_verifier)
    }

    /// Exchange Google authorization code for tokens.
    pub async fn google_exchange(
        &self,
        code: &str,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<
        oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
        AppError,
    > {
        self.google
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| AppError::internal(format!("Google token exchange: {e}")))
    }

    /// Fetch Google user info.
    pub async fn google_userinfo(&self, access_token: &str) -> Result<GoogleUserInfo, AppError> {
        let info = self
            .http
            .get("https://www.googleapis.com/oauth2/v3/userinfo")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("Google userinfo request: {e}")))?
            .json::<GoogleUserInfo>()
            .await
            .map_err(|e| AppError::internal(format!("Google userinfo parse: {e}")))?;
        Ok(info)
    }

    /// Build Microsoft authorization URL with PKCE.
    pub fn microsoft_auth_url(&self) -> (String, CsrfToken, PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state) = self
            .microsoft
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new("offline_access".to_string()))
            .add_scope(Scope::new("Calendars.ReadWrite".to_string()))
            .add_scope(Scope::new("User.Read".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();
        (url.to_string(), state, pkce_verifier)
    }

    /// Exchange Microsoft authorization code for tokens.
    pub async fn microsoft_exchange(
        &self,
        code: &str,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<
        oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
        AppError,
    > {
        self.microsoft
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| AppError::internal(format!("Microsoft token exchange: {e}")))
    }

    /// Fetch Microsoft user info from Graph API.
    pub async fn microsoft_userinfo(
        &self,
        access_token: &str,
    ) -> Result<MicrosoftUserInfo, AppError> {
        let info = self
            .http
            .get("https://graph.microsoft.com/v1.0/me")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("Microsoft userinfo request: {e}")))?
            .json::<MicrosoftUserInfo>()
            .await
            .map_err(|e| AppError::internal(format!("Microsoft userinfo parse: {e}")))?;
        Ok(info)
    }
}
