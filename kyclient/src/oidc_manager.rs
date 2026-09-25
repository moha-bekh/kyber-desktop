// Project Kyber: oidc_manager.rs
// Copyright © 2022-2026 Kyber SAS
// SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0-or-later
//
// This file is both under dual license: AGPLv3 and a Commercial one.
//
// ----
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! OIDC Authorization Code Flow with PKCE.
//!
//! Drives the interactive PKCE flow and background token refresh for kyclient,
//! using [`kyc`] for the protocol layer.

use std::fmt;
use std::time::Duration;

use log::{error, info, warn};
use log4ky::notice;
use winit::event_loop::EventLoopProxy;

use crate::error::{Error, Result};
use crate::event_loop::UserEvent;

/// Total timeout for a single kyhub request, Connect or refresh-connection.
/// Bounds the blocking `.send()`/`.json()` so a slow or half-open kyhub cannot
/// stall the refresh task forever (the session would silently stop renewing).
const KYHUB_EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Overrides [`KYHUB_EXCHANGE_TIMEOUT`] so a hung refresh leaves time to retry.
const KYHUB_REFRESH_TIMEOUT: Duration = Duration::from_secs(8);

/// How long before exp a kyhub machine token is renewed. The token lives about
/// 60 s, so the IdP lead of 60 s would renew it at once, again and again.
const KYHUB_REFRESH_LEAD_SECS: u64 = 30;

/// Public kyhub client the user signs in with before `:connect`, set at build
/// time with `KYHUB_CLIENT_ID`.
const KYHUB_PUBLIC_CLIENT_ID: &str = match option_env!("KYHUB_CLIENT_ID") {
    Some(id) => id,
    None => "kyhub-portal",
};

/// kyhub connection config for token exchange.
///
/// When set, the client signs the user in to kyhub and connects to the
/// controller with the machine-scoped token from `:connect`. Required when the
/// controller enforces `audience` in its OIDC config.
#[derive(Clone, Debug)]
pub(crate) struct KyhubConfig {
    /// Base URL of the kyhub API.
    pub(crate) url: String,
    /// Machine UUID, used as the `/machines/{id}` path segment. kyhub derives
    /// the exchange audience (the machine's IdP client ID) from it
    /// server-side; the client never sends an audience itself.
    pub(crate) machine_id: String,
    /// kyhub's OIDC issuer, where the user signs in.
    pub(crate) issuer: String,
}

/// A configured kyhub endpoint together with the HTTP client used to reach it.
/// Only `OidcManager::new` builds one, after `require_secure_kyhub_url` accepted
/// the base URL.
#[derive(Clone)]
struct KyhubClient {
    config: KyhubConfig,
    http: reqwest::Client,
}

#[derive(Clone)]
pub(crate) struct OidcResult {
    /// OAuth2 access token for API calls (UMA2, etc.).
    pub(crate) access_token: String,
    /// `None` when the provider does not issue refresh tokens.
    pub(crate) refresh_token: Option<String>,
    /// Unix timestamp when the token expires: the ID token for a PKCE sign-in,
    /// the machine token itself for kyhub.
    pub(crate) exp: u64,
}

/// Tokens stay out of logs: `EventLoopClosed` prints the event it carries.
impl fmt::Debug for OidcResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OidcResult")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("exp", &self.exp)
            .finish()
    }
}

fn to_result(tokens: kyc::OidcTokens) -> Result<OidcResult> {
    let exp = tokens
        .exp
        .ok_or_else(|| Error::Oidc(kyc::Error::Jwt("missing exp claim".to_owned())))?;

    Ok(OidcResult {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        exp,
    })
}

/// Exchange a portal token for a machine-scoped token via kyhub Connect.
///
/// On success the returned `OidcResult` carries the exchanged token (short-lived,
/// `aud=machine_client_id`) and kyhub's refresh token for `:refresh-connection`.
///
/// Failure is hard: if the exchange fails the error propagates. The caller must
/// not fall back to the portal token because that would silently downgrade to
/// no audience validation.
async fn kyhub_connect(kyhub: &KyhubClient, portal_token: &str) -> Result<OidcResult> {
    #[derive(serde::Deserialize)]
    struct ConnectResponse {
        credential: Option<Credential>,
    }

    #[derive(serde::Deserialize)]
    struct Credential {
        auth_token: String,
        auth_refresh_token: Option<String>,
    }

    let url = format!(
        "{}/machines/{}:connect",
        kyhub.config.url.trim_end_matches('/'),
        kyhub.config.machine_id
    );
    let resp = kyhub
        .http
        .post(&url)
        .bearer_auth(portal_token)
        .send()
        .await
        .map_err(|e| kyc::Error::TokenExchange(format!("{e:?}")))?;

    let status = resp.status();
    if !status.is_success() {
        // kyhub answers 404 for a machine the user may not see, so 403 and
        // 404 both mean no access, not a missing machine.
        let msg = if matches!(
            status,
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND
        ) {
            format!(
                "no access to machine {} (kyhub answered HTTP {status})",
                kyhub.config.machine_id
            )
        } else {
            format!("kyhub Connect returned HTTP {status}")
        };
        return Err(kyc::Error::TokenExchange(msg).into());
    }

    let body: ConnectResponse = resp
        .json()
        .await
        .map_err(|e| kyc::Error::TokenExchange(format!("{e:?}")))?;

    let credential = body.credential.ok_or_else(|| {
        kyc::Error::TokenExchange("kyhub Connect returned no credential".to_owned())
    })?;

    credential_result(credential.auth_token, credential.auth_refresh_token)
}

/// Build an [`OidcResult`] from a kyhub token pair. `exp` is read from the
/// access token's own claim, the same value the controller validates against;
/// the refresh token, if any, is kept for the next renewal.
fn credential_result(auth_token: String, auth_refresh_token: Option<String>) -> Result<OidcResult> {
    let exp = kyc::jwt::parse_exp(&auth_token).map_err(Error::from)?;
    Ok(OidcResult {
        access_token: auth_token,
        refresh_token: auth_refresh_token,
        exp,
    })
}

/// Sign the user in to kyhub, then exchange that token at `:connect`.
async fn run_kyhub_flow(kyhub: &KyhubClient) -> Result<OidcResult> {
    let portal = run_oidc_flow(&kyhub.config.issuer, KYHUB_PUBLIC_CLIENT_ID).await?;
    kyhub_connect(kyhub, &portal.access_token).await
}

async fn run_oidc_flow(issuer: &str, client_id: &str) -> Result<OidcResult> {
    let flow = kyc::PkceFlow::start(issuer, client_id).await?;

    let url = flow.auth_url().to_owned();

    notice!(
        "\nAttempting to open authentication page in your browser.\n\
         If it doesn't open, navigate to:\n  {url}"
    );

    info!("Opening browser for OIDC authentication");
    match tokio::task::spawn_blocking(move || kyc::browser::open_url(&url)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            warn!("Failed to open browser: {e}. Use the URL above to authenticate manually.")
        }
        Err(_) => warn!("Browser task panicked. Use the URL above to authenticate manually."),
    }

    notice!("Waiting for authentication...");

    let tokens = flow.wait_for_tokens().await?;

    to_result(tokens)
}

fn time_until_refresh(exp: u64) -> Duration {
    kyc::jwt::time_until_refresh(exp)
}

/// Time until a kyhub machine token is renewed: the lead before `exp`, but
/// never sooner than half of what is left. A client clock ahead of the IdP
/// shortens the visible lifetime, and a fixed lead would then renew at once,
/// again and again. At least one second, so a token that is already late
/// cannot spin the refresh loop.
fn time_until_kyhub_refresh(exp: u64) -> Duration {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let left = exp.saturating_sub(now);
    let secs = left.saturating_sub(KYHUB_REFRESH_LEAD_SECS).max(left / 2);
    Duration::from_secs(secs.max(1))
}

/// How the background task obtains the next token when a refresh fires.
#[derive(Clone)]
enum RefreshStrategy {
    /// Interactive PKCE tokens: refresh against the IdP.
    Pkce { issuer: String, client_id: String },
    /// kyhub machine tokens: redeem the refresh token at kyhub
    /// `:refresh-connection`. It is kyhub's refresh token, so the client cannot
    /// refresh directly against the IdP. Seeded by [`OidcManager::seed_kyhub`].
    Kyhub,
}

enum OidcState {
    Idle,
    Running {
        strategy: RefreshStrategy,
        refresh_token: Option<String>,
        cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
        flow_active: bool,
    },
}

pub struct OidcManager {
    state: OidcState,
    rt: Option<tokio::runtime::Runtime>,
    kyhub: Option<KyhubClient>,
}

impl Drop for OidcManager {
    fn drop(&mut self) {
        self.cancel_refresh();
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

fn send_oidc_result(proxy: &EventLoopProxy<UserEvent>, result: OidcResult) {
    if let Err(e) = proxy.send_event(UserEvent::OidcTokenReceived(result)) {
        error!("Failed to send OIDC result: {e}");
    }
}

/// Run `op` behind the retry schedule `delays`, in seconds. On the first success
/// the result is delivered via `proxy`; if every attempt fails, `OidcFlowFailed`
/// is signalled. `what` labels the operation in log lines.
async fn refresh_with_backoff<F, Fut>(
    what: &str,
    delays: &[u64],
    proxy: EventLoopProxy<UserEvent>,
    mut op: F,
) where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<OidcResult>>,
{
    let mut last_error = None;
    for i in 0..=delays.len() {
        if i > 0 {
            let backoff = delays[i - 1];
            info!("Retrying {what} in {backoff} seconds...");
            tokio::time::sleep(Duration::from_secs(backoff)).await;
        }

        match op().await {
            Ok(oidc) => {
                send_oidc_result(&proxy, oidc);
                return;
            }
            Err(e @ Error::KyhubRejected(_)) => {
                // A retry cannot change a 4xx: give up now.
                last_error = Some(e);
                break;
            }
            Err(e) => {
                warn!("{what} failed: {e:?}");
                last_error = Some(e);
            }
        }
    }
    warn!("{what} failed for good: {last_error:?}");
    let _ = proxy.send_event(UserEvent::OidcFlowFailed);
}

async fn refresh_oidc_with_retry(
    issuer: String,
    client_id: String,
    refresh_token: Option<String>,
    proxy: EventLoopProxy<UserEvent>,
) {
    const RETRY_DELAYS: [u64; 3] = [30, 60, 120];

    let Some(token) = refresh_token else {
        let _ = proxy.send_event(UserEvent::OidcFlowFailed);
        return;
    };

    let (issuer, client_id, token) = (&issuer, &client_id, &token);
    refresh_with_backoff(
        "silent OIDC refresh",
        &RETRY_DELAYS,
        proxy,
        move || async move {
            kyc::refresh_token(issuer, client_id, token)
                .await
                .map_err(Into::into)
                .and_then(to_result)
        },
    )
    .await;
}

/// Reject a kyhub base URL that would carry the portal or refresh token over
/// plaintext, except loopback dev hosts (`localhost`, `*.localhost`, and the
/// loopback IP ranges `127.0.0.0/8` / `::1`).
fn require_secure_kyhub_url(base_url: &str) -> Result<()> {
    let parsed =
        url::Url::parse(base_url).map_err(|e| Error::Config(format!("invalid kyhub URL: {e}")))?;
    let is_loopback = match parsed.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(host)) => host == "localhost" || host.ends_with(".localhost"),
        None => false,
    };
    if parsed.scheme() == "https" || (parsed.scheme() == "http" && is_loopback) {
        return Ok(());
    }
    Err(Error::Config(format!(
        "refusing to send tokens to insecure kyhub URL scheme: {}",
        parsed.scheme()
    )))
}

/// Redeem a portal-issued refresh token at kyhub `:refresh-connection` for a
/// fresh machine-audienced access token. The refresh token rotates, so the new
/// one is threaded back for the next cycle. `exp` comes from the returned access
/// token's own claim (what the controller validates against).
async fn refresh_connection(kyhub: &KyhubClient, refresh_token: &str) -> Result<OidcResult> {
    #[derive(serde::Serialize)]
    struct RefreshRequest<'a> {
        refresh_token: &'a str,
    }

    #[derive(serde::Deserialize)]
    struct RefreshResponse {
        auth_token: String,
        auth_refresh_token: Option<String>,
    }

    let url = format!(
        "{}/machines/{}:refresh-connection",
        kyhub.config.url.trim_end_matches('/'),
        kyhub.config.machine_id
    );
    let resp = kyhub
        .http
        .post(&url)
        .timeout(KYHUB_REFRESH_TIMEOUT)
        .json(&RefreshRequest { refresh_token })
        .send()
        .await
        .map_err(|e| kyc::Error::TokenExchange(format!("{e:?}")))?;

    let status = resp.status();
    if !status.is_success() {
        // The refresh token is spent, or the machine is gone or disabled:
        // a 4xx will not change on retry. 429 is the exception.
        if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::KyhubRejected(status));
        }
        return Err(kyc::Error::TokenExchange(format!(
            "kyhub refresh-connection returned HTTP {status}"
        ))
        .into());
    }

    let body: RefreshResponse = resp
        .json()
        .await
        .map_err(|e| kyc::Error::TokenExchange(format!("{e:?}")))?;

    credential_result(body.auth_token, body.auth_refresh_token)
}

/// kyhub refresh loop: redeem the refresh token at kyhub
/// `:refresh-connection`, retrying with backoff. Mirrors
/// [`refresh_oidc_with_retry`] but talks only to kyhub. On exhaustion (or an
/// expired refresh token) it signals failure and the session ends at the
/// current access token's expiry.
async fn refresh_kyhub_with_retry(
    kyhub: Option<KyhubClient>,
    refresh_token: Option<String>,
    proxy: EventLoopProxy<UserEvent>,
) {
    const RETRY_DELAYS: [u64; 2] = [5, 10];

    let (Some(kyhub), Some(token)) = (kyhub, refresh_token) else {
        warn!("kyhub refresh without kyhub config or refresh token");
        let _ = proxy.send_event(UserEvent::OidcFlowFailed);
        return;
    };

    let (kyhub, token) = (&kyhub, &token);
    refresh_with_backoff("kyhub refresh", &RETRY_DELAYS, proxy, move || async move {
        refresh_connection(kyhub, token).await
    })
    .await;
}

impl OidcManager {
    pub fn new(kyhub_config: Option<KyhubConfig>) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("oidc")
            .enable_io()
            .enable_time()
            .build()
            .map_err(|e| Error::Config(format!("Failed to create async runtime: {e}")))?;
        let kyhub = match kyhub_config {
            Some(config) => {
                require_secure_kyhub_url(&config.url)?;
                let http = reqwest::Client::builder()
                    .timeout(KYHUB_EXCHANGE_TIMEOUT)
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .map_err(|e| {
                        Error::Config(format!("Failed to create kyhub HTTP client: {e}"))
                    })?;
                Some(KyhubClient { config, http })
            }
            None => None,
        };
        Ok(Self {
            state: OidcState::Idle,
            rt: Some(rt),
            kyhub,
        })
    }

    fn rt(&self) -> &tokio::runtime::Runtime {
        self.rt.as_ref().expect("runtime present until drop")
    }

    pub fn is_flow_active(&self) -> bool {
        matches!(
            self.state,
            OidcState::Running {
                flow_active: true,
                ..
            }
        )
    }

    pub fn start_flow(
        &mut self,
        issuer: String,
        client_id: String,
        proxy: EventLoopProxy<UserEvent>,
    ) {
        // In kyhub mode the challenge means the machine token was refused: sign
        // in to kyhub again instead of using the controller's IdP client.
        if let Some(kyhub) = self.kyhub.clone() {
            self.state = OidcState::Running {
                strategy: RefreshStrategy::Kyhub,
                refresh_token: None,
                cancel_tx: None,
                flow_active: true,
            };
            self.rt().spawn(async move {
                match run_kyhub_flow(&kyhub).await {
                    Ok(oidc) => send_oidc_result(&proxy, oidc),
                    Err(e) => {
                        warn!("kyhub sign-in failed: {e:?}");
                        let _ = proxy.send_event(UserEvent::OidcFlowFailed);
                    }
                }
            });
            return;
        }

        self.state = OidcState::Running {
            strategy: RefreshStrategy::Pkce {
                issuer: issuer.clone(),
                client_id: client_id.clone(),
            },
            refresh_token: None,
            cancel_tx: None,
            flow_active: true,
        };
        self.rt().spawn(async move {
            match run_oidc_flow(&issuer, &client_id).await {
                Ok(oidc) => send_oidc_result(&proxy, oidc),
                Err(e) => {
                    warn!("OIDC flow failed: {e:?}");
                    let _ = proxy.send_event(UserEvent::OidcFlowFailed);
                }
            }
        });
    }

    /// Sign in to kyhub and get the machine token, blocking until done, then
    /// schedule its renewal. Runs before the client connects, so the controller
    /// never sends a challenge.
    pub fn kyhub_login(&mut self, proxy: EventLoopProxy<UserEvent>) -> Result<OidcResult> {
        let kyhub = self
            .kyhub
            .clone()
            .ok_or_else(|| Error::Config("kyhub login without kyhub config".to_owned()))?;
        let machine = self.rt().block_on(run_kyhub_flow(&kyhub))?;
        self.start_kyhub_renewal(&machine, proxy);
        Ok(machine)
    }

    fn start_kyhub_renewal(&mut self, machine: &OidcResult, proxy: EventLoopProxy<UserEvent>) {
        match &machine.refresh_token {
            Some(token) => {
                self.seed_kyhub(token.clone());
                self.schedule_refresh(machine.exp, proxy);
            }
            None => warn!("no kyhub refresh token, the session ends when the token expires"),
        }
    }

    pub fn on_token_received(&mut self, oidc: &OidcResult) {
        if let OidcState::Running {
            refresh_token,
            flow_active,
            ..
        } = &mut self.state
        {
            *flow_active = false;
            if oidc.refresh_token.is_some() {
                *refresh_token = oidc.refresh_token.clone();
            }
        }
    }

    pub fn on_flow_failed(&mut self) {
        if let OidcState::Running { flow_active, .. } = &mut self.state {
            *flow_active = false;
        }
    }

    /// Seed the manager for the kyhub refresh path: only the refresh token is
    /// stored, ready for [`Self::schedule_refresh`].
    pub fn seed_kyhub(&mut self, refresh_token: String) {
        self.state = OidcState::Running {
            strategy: RefreshStrategy::Kyhub,
            refresh_token: Some(refresh_token),
            cancel_tx: None,
            flow_active: false,
        };
    }

    pub fn schedule_refresh(&mut self, exp: u64, proxy: EventLoopProxy<UserEvent>) {
        self.cancel_refresh();

        let OidcState::Running {
            strategy,
            refresh_token,
            cancel_tx,
            ..
        } = &mut self.state
        else {
            return;
        };

        let strategy = strategy.clone();
        let refresh_token = refresh_token.clone();
        let kyhub = self.kyhub.clone();

        let delay = match strategy {
            RefreshStrategy::Pkce { .. } => time_until_refresh(exp),
            RefreshStrategy::Kyhub => time_until_kyhub_refresh(exp),
        };
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        *cancel_tx = Some(tx);

        self.rt().spawn(async move {
            tokio::select! {
                _ = &mut rx => {}
                _ = tokio::time::sleep(delay) => {
                    match strategy {
                        RefreshStrategy::Pkce { issuer, client_id } => {
                            refresh_oidc_with_retry(issuer, client_id, refresh_token, proxy).await;
                        }
                        RefreshStrategy::Kyhub => {
                            refresh_kyhub_with_retry(kyhub, refresh_token, proxy).await;
                        }
                    }
                }
            }
        });
    }

    pub fn cancel_refresh(&mut self) {
        if let OidcState::Running { cancel_tx, .. } = &mut self.state {
            cancel_tx.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_jwt_exp(jwt: &str) -> Result<u64> {
        kyc::jwt::parse_exp(jwt).map_err(Into::into)
    }

    fn make_jwt(payload_json: &str) -> String {
        use base64::Engine;
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256"}"#);
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        format!("{header}.{payload}.signature")
    }

    #[test]
    fn parse_jwt_exp_valid() {
        let jwt = make_jwt(r#"{"sub":"user","exp":9999999999}"#);
        assert_eq!(parse_jwt_exp(&jwt).unwrap(), 9999999999);
    }

    #[test]
    fn parse_jwt_exp_missing_claim() {
        let jwt = make_jwt(r#"{"sub":"user"}"#);
        assert!(parse_jwt_exp(&jwt).is_err());
    }

    #[test]
    fn parse_jwt_exp_non_jwt_input() {
        assert!(parse_jwt_exp("onlyone").is_err());
        assert!(parse_jwt_exp("not.a.jwt.with.too.many.dots").is_err());
    }

    #[test]
    fn time_until_refresh_expired() {
        assert_eq!(time_until_refresh(0), Duration::ZERO);
        assert_eq!(time_until_refresh(60), Duration::ZERO);
    }

    #[test]
    fn time_until_refresh_future() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let exp = now + 3600;
        let d = time_until_refresh(exp);
        assert!(d.as_secs() >= 3535 && d.as_secs() <= 3540);
    }

    #[test]
    fn time_until_kyhub_refresh_leads_by_30s_and_never_zero() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let d = time_until_kyhub_refresh(now + 60);
        assert!(d.as_secs() >= 29 && d.as_secs() <= 30);
        // A short or skewed lifetime renews at half of what is left.
        let d = time_until_kyhub_refresh(now + 30);
        assert!(d.as_secs() >= 14 && d.as_secs() <= 15);
        assert_eq!(time_until_kyhub_refresh(0), Duration::from_secs(1));
    }

    #[test]
    fn secure_kyhub_url_accepts_https_any_host() {
        assert!(require_secure_kyhub_url("https://api.kyhub.example.com/api/v1").is_ok());
    }

    #[test]
    fn secure_kyhub_url_accepts_http_loopback() {
        assert!(require_secure_kyhub_url("http://api.kyhub.localhost/api/v1").is_ok());
        assert!(require_secure_kyhub_url("http://localhost:8080/api/v1").is_ok());
        assert!(require_secure_kyhub_url("http://127.0.0.1:8080/api/v1").is_ok());
        assert!(require_secure_kyhub_url("http://[::1]:8080/api/v1").is_ok());
    }

    #[test]
    fn secure_kyhub_url_rejects_http_public_host() {
        assert!(require_secure_kyhub_url("http://api.kyhub.example.com/api/v1").is_err());
        // Names that only look local, and IPs outside the loopback ranges.
        assert!(require_secure_kyhub_url("http://localhost.attacker.example/api/v1").is_err());
        assert!(require_secure_kyhub_url("http://127.0.0.1.nip.io/api/v1").is_err());
        assert!(require_secure_kyhub_url("http://[::ffff:127.0.0.1]/api/v1").is_err());
        assert!(require_secure_kyhub_url("http://0.0.0.0/api/v1").is_err());
    }

    #[test]
    fn secure_kyhub_url_rejects_invalid() {
        assert!(require_secure_kyhub_url("not a url").is_err());
    }

    #[test]
    fn credential_result_reads_exp_and_threads_rotated_token() {
        let jwt = make_jwt(r#"{"sub":"user","exp":9999999999}"#);
        let out = credential_result(jwt.clone(), Some("rotated".to_owned())).unwrap();
        assert_eq!(out.access_token, jwt);
        assert_eq!(out.exp, 9999999999);
        assert_eq!(out.refresh_token.as_deref(), Some("rotated"));
    }

    #[test]
    fn credential_result_allows_absent_rotated_token() {
        let jwt = make_jwt(r#"{"sub":"user","exp":9999999999}"#);
        let out = credential_result(jwt, None).unwrap();
        assert!(out.refresh_token.is_none());
    }

    #[test]
    fn credential_result_rejects_token_without_exp() {
        let jwt = make_jwt(r#"{"sub":"user"}"#);
        assert!(credential_result(jwt, None).is_err());
    }
}
