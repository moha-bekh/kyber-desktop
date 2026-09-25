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

#[derive(Clone)]
pub(crate) struct OidcResult {
    /// OAuth2 access token for API calls (UMA2, etc.).
    pub(crate) access_token: String,
    /// `None` when the provider does not issue refresh tokens.
    pub(crate) refresh_token: Option<String>,
    /// Unix timestamp when the ID token expires.
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

enum OidcState {
    Idle,
    Running {
        issuer: String,
        client_id: String,
        refresh_token: Option<String>,
        cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
        flow_active: bool,
    },
}

pub struct OidcManager {
    state: OidcState,
    rt: Option<tokio::runtime::Runtime>,
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

async fn refresh_oidc_with_retry(
    issuer: String,
    client_id: String,
    refresh_token: Option<String>,
    proxy: EventLoopProxy<UserEvent>,
) {
    const RETRY_DELAYS: [u64; 3] = [30, 60, 120];

    if let Some(token) = refresh_token {
        let mut last_error = None;
        for i in 0..=RETRY_DELAYS.len() {
            if i > 0 {
                let backoff = RETRY_DELAYS[i - 1];
                info!("Retrying silent OIDC refresh in {backoff} seconds...");
                tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
            }

            match kyc::refresh_token(&issuer, &client_id, &token)
                .await
                .map_err(Into::into)
                .and_then(to_result)
            {
                Ok(oidc) => {
                    send_oidc_result(&proxy, oidc);
                    return;
                }
                Err(e) => {
                    warn!("Silent OIDC refresh failed: {e:?}");
                    last_error = Some(e);
                }
            }
        }
        warn!("Silent OIDC refresh failed after retries: {last_error:?}");
    }

    let _ = proxy.send_event(UserEvent::OidcFlowFailed);
}

impl OidcManager {
    pub fn new() -> Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("oidc")
            .enable_io()
            .enable_time()
            .build()
            .map_err(|e| Error::Config(format!("Failed to create async runtime: {e}")))?;
        Ok(Self {
            state: OidcState::Idle,
            rt: Some(rt),
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
        self.state = OidcState::Running {
            issuer: issuer.clone(),
            client_id: client_id.clone(),
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

    pub fn schedule_refresh(&mut self, exp: u64, proxy: EventLoopProxy<UserEvent>) {
        self.cancel_refresh();

        let OidcState::Running {
            issuer,
            client_id,
            refresh_token,
            cancel_tx,
            ..
        } = &mut self.state
        else {
            return;
        };

        let (issuer, client_id, refresh_token) =
            (issuer.clone(), client_id.clone(), refresh_token.clone());

        let delay = time_until_refresh(exp);
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        *cancel_tx = Some(tx);

        self.rt().spawn(async move {
            tokio::select! {
                _ = &mut rx => {}
                _ = tokio::time::sleep(delay) => {
                    refresh_oidc_with_retry(issuer, client_id, refresh_token, proxy).await;
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
}
