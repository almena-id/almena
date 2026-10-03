//! Asking the Almena wallet: the request the API makes is shown as a QR code
//! and a link, then polled until the wallet has answered (or it expires).
//! Signing in, linking the wallet, and every signature the tenant makes
//! (publishing, an identity's log, a credential) go this way.

use std::thread;
use std::time::Duration;

use anyhow::bail;
use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
use serde_json::{Value, json};

use crate::client::{ApiError, Client};
use crate::output::notice;

/// Who asks: the wallet answers the CLI, and its sheet says so.
pub const CLIENT: &str = "cli";

pub const POLL_EVERY: Duration = Duration::from_secs(2);

/// The body of a request for the wallet to sign: the language of its sheet,
/// and that the CLI asks.
pub fn asking(locale: &str) -> Value {
    json!({"locale": locale, "client": CLIENT})
}

/// Shows the request (`{id, deep_link, poll, …}`) and waits for its outcome:
/// `signed_in` (with `session`), `linked`, `taken` or `signed`.
pub fn wait(api: &Client, request: &Value, what: &str) -> anyhow::Result<Value> {
    let (Some(id), Some(link), Some(poll)) = (
        request["id"].as_str(),
        request["deep_link"].as_str(),
        request["poll"].as_str(),
    ) else {
        bail!("the API's wallet request is missing its id, link or poll secret");
    };
    show(link, what);
    loop {
        let result = match api.post(
            &["auth", "wallet", "requests", id, "result"],
            &json!({"poll": poll}),
        ) {
            Ok(result) => result,
            Err(error) => {
                if error
                    .downcast_ref::<ApiError>()
                    .is_some_and(|e| e.code == "request_expired")
                {
                    bail!("the wallet did not answer in time; try again");
                }
                return Err(error);
            }
        };
        if result["status"] != "pending" {
            return Ok(result);
        }
        thread::sleep(POLL_EVERY);
    }
}

/// The link as a QR code and as text, and that it is being waited for.
pub fn show(link: &str, what: &str) {
    notice(format!("Scan with your Almena wallet to {what}:\n"));
    if let Ok(code) = QrCode::new(link.as_bytes()) {
        let image = code
            .render::<Dense1x2>()
            .dark_color(Dense1x2::Light)
            .light_color(Dense1x2::Dark)
            .quiet_zone(true)
            .build();
        notice(image);
    }
    notice(format!("\nOr open on this device: {link}"));
    notice("Waiting for the wallet… (Ctrl-C to stop)");
}
