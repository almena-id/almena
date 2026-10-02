//! The `almena` binary against a stand-in API (wiremock): each command sends
//! what the API expects and prints what it answers.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{Value, json};
use tempfile::TempDir;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const TOKEN: &str = "the-session";

struct Env {
    server: MockServer,
    dir: TempDir,
}

impl Env {
    /// Signed in (a session kept in the file store), with one tenant.
    async fn new() -> Self {
        let env = Self::signed_out().await;
        let credentials = format!(
            "[profiles.default]\ntoken = \"{TOKEN}\"\nkind = \"session\"\napi_url = \"{}\"\n",
            env.server.uri()
        );
        std::fs::write(env.dir.path().join("credentials.toml"), credentials).unwrap();
        env.on(
            "GET",
            "/api/v1/tenants",
            json!([{"id": TENANT, "name": "Acme", "role": "admin"}]),
        )
        .await;
        env
    }

    async fn signed_out() -> Self {
        Self {
            server: MockServer::start().await,
            dir: TempDir::new().unwrap(),
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("almena").unwrap();
        cmd.env_clear()
            .env("ALMENA_CONFIG_DIR", self.dir.path())
            .env("ALMENA_CREDENTIAL_STORE", "file")
            .env("ALMENA_API_URL", self.server.uri());
        cmd
    }

    /// Answers `method path` with `body`, signed in, any number of times.
    async fn on(&self, method_: &str, path_: &str, body: Value) {
        Mock::given(method(method_))
            .and(path(path_))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&self.server)
            .await;
    }

    /// Expects `method path` exactly once, with this JSON body.
    async fn expect(&self, method_: &str, path_: &str, sent: Value, body: Value) {
        Mock::given(method(method_))
            .and(path(path_))
            .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
            .and(body_json(sent))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&self.server)
            .await;
    }

    async fn refuse(&self, method_: &str, path_: &str, status: u16, detail: Value) {
        Mock::given(method(method_))
            .and(path(path_))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({"detail": detail})))
            .mount(&self.server)
            .await;
    }

    /// A wallet request the API makes, already answered with `status`.
    async fn wallet(&self, made_by: &str, status: &str) {
        let request = json!({
            "id": "r1", "request_uri": "u", "poll": "p1",
            "deep_link": "almena://auth?request_uri=u", "expires_at": "2026-10-03T00:00:00Z",
        });
        self.expect(
            "POST",
            made_by,
            json!({"locale": "en", "client": "cli"}),
            request,
        )
        .await;
        self.expect(
            "POST",
            "/api/v1/auth/wallet/requests/r1/result",
            json!({"poll": "p1"}),
            json!({"status": status}),
        )
        .await;
    }
}

fn tenant(rest: &str) -> String {
    format!("/api/v1/tenants/{TENANT}{rest}")
}

// --- auth -------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn without_a_sign_in_commands_exit_3() {
    let env = Env::signed_out().await;
    env.cmd()
        .args(["tenant", "list"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("almena auth login"));
}

#[tokio::test(flavor = "multi_thread")]
async fn status_says_who_is_signed_in() {
    let env = Env::new().await;
    env.on(
        "GET",
        "/api/v1/auth/me",
        json!({"id": "u1", "email": "ada@example.org", "alias": null}),
    )
    .await;
    env.cmd()
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("ada@example.org").and(predicate::str::contains("session")),
        );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_from_stdin_is_kept_after_checking_it() {
    let env = Env::signed_out().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .and(header("authorization", "Bearer almena_abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"email": "ci@example.org"})))
        .expect(1)
        .mount(&env.server)
        .await;
    env.cmd()
        .args(["auth", "login", "--with-token"])
        .write_stdin("almena_abc\n")
        .assert()
        .success()
        .stderr(predicate::str::contains("Signed in as ci@example.org"));
    let kept = std::fs::read_to_string(env.dir.path().join("credentials.toml")).unwrap();
    assert!(kept.contains("almena_abc") && kept.contains("api_token"));
}

#[tokio::test(flavor = "multi_thread")]
async fn almena_token_signs_in_without_anything_kept() {
    let env = Env::signed_out().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me/tokens"))
        .and(header("authorization", "Bearer from-env"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"items": []})))
        .expect(1)
        .mount(&env.server)
        .await;
    env.cmd()
        .env("ALMENA_TOKEN", "from-env")
        .args(["token", "list"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn logout_ends_the_session_and_forgets_it() {
    let env = Env::new().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/auth/logout"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&env.server)
        .await;
    env.cmd().args(["auth", "logout"]).assert().success();
    let kept = std::fs::read_to_string(env.dir.path().join("credentials.toml")).unwrap();
    assert!(!kept.contains(TOKEN));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wallet_sign_in_keeps_the_session() {
    let env = Env::signed_out().await;
    let request = json!({"id": "r1", "deep_link": "almena://auth?request_uri=u", "poll": "p1"});
    Mock::given(method("POST"))
        .and(path("/api/v1/auth/wallet/requests"))
        .and(body_json(
            json!({"purpose": "sign_in", "locale": "en", "client": "cli"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(request))
        .mount(&env.server)
        .await;
    let session =
        json!({"token": "s2", "expires_at": "2026-10-03T12:00:00Z", "user": {"email": null}});
    env.on(
        "POST",
        "/api/v1/auth/wallet/requests/r1/result",
        json!({"status": "signed_in", "session": session}),
    )
    .await;
    env.cmd()
        .args(["auth", "login", "--wallet"])
        .assert()
        .success()
        .stderr(predicate::str::contains("almena://auth?request_uri=u"));
    let kept = std::fs::read_to_string(env.dir.path().join("credentials.toml")).unwrap();
    assert!(kept.contains("\"s2\""));
}

// --- account and tokens -----------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn account_update_sets_or_clears_the_alias() {
    let env = Env::new().await;
    env.expect(
        "PATCH",
        "/api/v1/auth/me",
        json!({"alias": "Ada"}),
        json!({"alias": "Ada"}),
    )
    .await;
    env.cmd()
        .args(["account", "update", "--alias", "Ada"])
        .assert()
        .success();
    let env = Env::new().await;
    env.expect(
        "PATCH",
        "/api/v1/auth/me",
        json!({"alias": ""}),
        json!({"alias": null}),
    )
    .await;
    env.cmd()
        .args(["account", "update", "--no-alias"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn account_shows_its_ways_in() {
    let env = Env::new().await;
    let accounts =
        json!([{"id": "a1", "provider": "almena", "did": "did:key:z6Mk", "email": null}]);
    env.on(
        "GET",
        "/api/v1/auth/me/ways-in",
        json!({"email": "ada@example.org", "accounts": accounts}),
    )
    .await;
    env.cmd()
        .args(["account", "ways-in"])
        .assert()
        .success()
        .stdout(predicate::str::contains("did:key:z6Mk"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wallet_is_linked_from_the_cli() {
    let env = Env::new().await;
    let request = json!({"id": "r1", "deep_link": "almena://auth?request_uri=u", "poll": "p1"});
    env.expect(
        "POST",
        "/api/v1/auth/wallet/requests",
        json!({"purpose": "link", "locale": "en", "client": "cli"}),
        request,
    )
    .await;
    env.on(
        "POST",
        "/api/v1/auth/wallet/requests/r1/result",
        json!({"status": "linked"}),
    )
    .await;
    env.cmd()
        .args(["account", "link-wallet"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Linked."));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_is_made_and_printed_once() {
    let env = Env::new().await;
    env.expect(
        "POST",
        "/api/v1/auth/me/tokens",
        json!({"name": "CI", "expires_in_days": 30}),
        json!({"id": "t1", "name": "CI", "token": "almena_secret", "expires_at": "2026-11-02"}),
    )
    .await;
    env.cmd()
        .args(["token", "create", "--name", "CI", "--days", "30"])
        .assert()
        .success()
        .stdout("almena_secret\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_asks_unless_yes() {
    let env = Env::new().await;
    env.cmd()
        .args(["token", "delete", "t1"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--yes"));
    Mock::given(method("DELETE"))
        .and(path("/api/v1/auth/me/tokens/t1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&env.server)
        .await;
    env.cmd()
        .args(["--yes", "token", "delete", "t1"])
        .assert()
        .success();
}

// --- tenants ----------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn the_only_tenant_is_the_one_used() {
    let env = Env::new().await;
    env.on(
        "GET",
        &tenant(""),
        json!({"id": TENANT, "name": "Acme", "role": "admin"}),
    )
    .await;
    env.cmd()
        .args(["tenant", "get"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Acme"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tenant_is_picked_by_name_and_remembered() {
    let env = Env::new().await;
    env.cmd().args(["tenant", "use", "Acme"]).assert().success();
    let config = std::fs::read_to_string(env.dir.path().join("config.toml")).unwrap();
    assert!(config.contains(TENANT));
    env.cmd()
        .args(["tenant", "use", "Nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no tenant of yours"));
}

#[tokio::test(flavor = "multi_thread")]
async fn tenant_update_sends_only_what_changes() {
    let env = Env::new().await;
    env.expect(
        "PATCH",
        &tenant(""),
        json!({"mediator_id": null, "signing_flow": "single_user", "signer_id": "u2"}),
        json!({"id": TENANT}),
    )
    .await;
    env.cmd()
        .args([
            "tenant",
            "update",
            "--no-mediator",
            "--signing-flow",
            "single-user",
            "--signer",
            "u2",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn tenant_health_lists_its_checks() {
    let env = Env::new().await;
    let checks = json!([{"check": "mediator", "done": false, "issue": "missing"}]);
    env.on(
        "GET",
        &tenant("/health"),
        json!({"score": 67, "checks": checks}),
    )
    .await;
    env.cmd()
        .args(["tenant", "health"])
        .assert()
        .success()
        .stderr(predicate::str::contains("67%"))
        .stdout(predicate::str::contains("missing"));
}

// --- issuers, verifiers, mediators, identities ------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn lists_follow_every_page_with_all() {
    let env = Env::new().await;
    Mock::given(method("GET"))
        .and(path(tenant("/issuers")))
        .and(query_param("cursor", "c2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"items": [{"id": "i2", "name": "Second"}], "next_cursor": null}),
            ),
        )
        .mount(&env.server)
        .await;
    Mock::given(method("GET"))
        .and(path(tenant("/issuers")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"items": [{"id": "i1", "name": "First"}], "next_cursor": "c2"}),
            ),
        )
        .mount(&env.server)
        .await;
    env.cmd()
        .args(["-o", "json", "issuer", "list", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("First").and(predicate::str::contains("Second")));
    env.cmd()
        .args(["issuer", "list"])
        .assert()
        .success()
        .stderr(predicate::str::contains("--cursor c2"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_issuer_is_registered() {
    let env = Env::new().await;
    env.expect(
        "POST",
        &tenant("/issuers"),
        json!({"name": "Academy", "description": null, "mediator_id": "m1"}),
        json!({"id": "i1", "name": "Academy"}),
    )
    .await;
    env.cmd()
        .args(["issuer", "create", "--name", "Academy", "--mediator", "m1"])
        .assert()
        .success()
        .stderr(predicate::str::contains("almena issuer publish i1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_update_sends_null_to_remove() {
    let env = Env::new().await;
    env.expect(
        "PATCH",
        &tenant("/verifiers/v1"),
        json!({"name": "Gate", "description": null}),
        json!({"id": "v1"}),
    )
    .await;
    env.cmd()
        .args([
            "verifier",
            "update",
            "v1",
            "--name",
            "Gate",
            "--no-description",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn publishing_asks_the_wallet_then_shows_it() {
    let env = Env::new().await;
    env.wallet(&tenant("/issuers/i1/publish"), "signed").await;
    env.on(
        "GET",
        &tenant("/issuers/i1"),
        json!({"id": "i1", "published_at": "2026-10-02T10:00:00Z"}),
    )
    .await;
    env.cmd()
        .args(["issuer", "publish", "i1"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Published."))
        .stdout(predicate::str::contains("2026-10-02T10:00:00Z"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_expired_wallet_request_is_said_so() {
    let env = Env::new().await;
    let request = json!({"id": "r1", "deep_link": "almena://auth?request_uri=u", "poll": "p1"});
    env.on("POST", &tenant("/mediators/m1/publish"), request)
        .await;
    env.refuse(
        "POST",
        "/api/v1/auth/wallet/requests/r1/result",
        410,
        json!("request_expired"),
    )
    .await;
    env.cmd()
        .args(["mediator", "publish", "m1"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("did not answer in time"));
}

#[tokio::test(flavor = "multi_thread")]
async fn unpublishing_and_signing_systems() {
    let env = Env::new().await;
    env.on(
        "POST",
        &tenant("/issuers/i1/unpublish"),
        json!({"id": "i1"}),
    )
    .await;
    env.cmd()
        .args(["issuer", "unpublish", "i1"])
        .assert()
        .success();
    env.expect(
        "PUT",
        &tenant("/issuers/i1/signing"),
        json!({"system": "single_user", "user_id": "u1"}),
        json!({"system": "single_user", "signer": {"id": "u1"}}),
    )
    .await;
    env.cmd()
        .args(["issuer", "signing", "set", "i1", "--user", "u1"])
        .assert()
        .success();
    env.expect(
        "PUT",
        &tenant("/verifiers/v1/signing"),
        json!({"system": null}),
        json!({"system": null}),
    )
    .await;
    env.cmd()
        .args(["verifier", "signing", "set", "v1", "--none"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn credential_types_map_to_forms() {
    let env = Env::new().await;
    env.expect(
        "PUT",
        &tenant("/issuers/i1/credential-types"),
        json!({"types": ["enrollment", "membership"], "forms": {"enrollment": "f1"}}),
        json!({"types": ["enrollment", "membership"], "forms": {"enrollment": "f1"}}),
    )
    .await;
    env.cmd()
        .args([
            "issuer",
            "credential-types",
            "set",
            "i1",
            "--type",
            "enrollment",
        ])
        .args(["--type", "membership", "--form", "enrollment=f1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("f1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn mediators_are_registered_and_changed() {
    let env = Env::new().await;
    env.expect(
        "POST",
        &tenant("/mediators"),
        json!({"name": "Relay", "subdomain": "relay", "domain_id": "d1", "public": true}),
        json!({"id": "m1", "url": "https://relay.acme.com"}),
    )
    .await;
    env.cmd()
        .args([
            "mediator",
            "create",
            "--name",
            "Relay",
            "--subdomain",
            "relay",
        ])
        .args(["--domain", "d1", "--public"])
        .assert()
        .success();
    env.expect(
        "PATCH",
        &tenant("/mediators/m1"),
        json!({"public": false}),
        json!({"id": "m1"}),
    )
    .await;
    env.cmd()
        .args(["mediator", "update", "m1", "--public", "false"])
        .assert()
        .success();
    env.on(
        "GET",
        &tenant("/mediator-choices"),
        json!([{"id": "m1", "name": "Relay", "own": true}]),
    )
    .await;
    env.cmd()
        .args(["mediator", "choices"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Relay"));
}

#[tokio::test(flavor = "multi_thread")]
async fn identities_are_registered_and_signed() {
    let env = Env::new().await;
    env.expect(
        "POST",
        &tenant("/identities"),
        json!({"name": "Ops"}),
        json!({"id": "n1"}),
    )
    .await;
    env.cmd()
        .args(["identity", "create", "--name", "Ops"])
        .assert()
        .success();
    env.wallet(&tenant("/identities/n1/sign"), "signed").await;
    env.on(
        "GET",
        &tenant("/identities/n1"),
        json!({"id": "n1", "signature": "signed"}),
    )
    .await;
    env.cmd()
        .args(["identity", "sign", "n1"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Signed."));
    env.on(
        "GET",
        &tenant("/signatures"),
        json!([{"id": "n2", "name": "Late", "signature": "outdated"}]),
    )
    .await;
    env.cmd()
        .args(["signature", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("outdated"));
}

// --- domains, members -------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_domain_is_linked_with_its_record() {
    let env = Env::new().await;
    let record = json!({"type": "TXT", "name": "_almena.acme.com", "value": "almena-verify=x"});
    env.expect(
        "POST",
        &tenant("/domains"),
        json!({"domain": "acme.com"}),
        json!({"id": "d1", "domain": "acme.com", "dns_record": record, "verified": false}),
    )
    .await;
    env.cmd()
        .args(["domain", "add", "acme.com"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "_almena.acme.com TXT \"almena-verify=x\"",
        ));
    env.on(
        "POST",
        &tenant("/domains/d1/check"),
        json!({"id": "d1", "verified": true}),
    )
    .await;
    env.cmd()
        .args(["domain", "check", "d1"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Verified."));
}

#[tokio::test(flavor = "multi_thread")]
async fn members_are_invited() {
    let env = Env::new().await;
    env.expect(
        "POST",
        &tenant("/invitations"),
        json!({"email": "bob@example.org", "role": "admin", "locale": "es"}),
        json!({"email": "bob@example.org", "role": "admin", "status": "invited"}),
    )
    .await;
    env.cmd()
        .args([
            "--locale",
            "es",
            "member",
            "invite",
            "--email",
            "bob@example.org",
            "--role",
            "admin",
        ])
        .assert()
        .success();
}

// --- forms, fields, catalogue -----------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_form_is_made_from_flags() {
    let env = Env::new().await;
    env.expect(
        "POST",
        &tenant("/forms"),
        json!({
            "name": {"en": "Enrolment", "es": "Matrícula"},
            "fields": [{"ref": "given_name", "required": true}, {"ref": "custom:no", "required": false}],
            "credentials": [{"type": "verified_email"}],
        }),
        json!({"id": "f1", "name": {"en": "Enrolment", "es": "Matrícula"}, "fields": []}),
    )
    .await;
    env.cmd()
        .args([
            "--locale",
            "es",
            "form",
            "create",
            "--name",
            "en=Enrolment",
            "--name",
            "es=Matrícula",
        ])
        .args(["--field", "given_name", "--optional-field", "custom:no"])
        .args(["--credential", "verified_email"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Matrícula"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_form_needs_a_name() {
    let env = Env::new().await;
    env.cmd()
        .args(["form", "create", "--field", "given_name"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("needs a name"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_form_verifies_what_is_read_from_a_file() {
    let env = Env::new().await;
    let file = env.dir.path().join("vp.json");
    let presented = json!({"vp_token": {"q": ["ey"]}, "nonce": "n", "audience": "a"});
    std::fs::write(&file, presented.to_string()).unwrap();
    env.expect(
        "POST",
        &tenant("/forms/f1/verify"),
        presented,
        json!({"valid": true}),
    )
    .await;
    env.cmd()
        .args(["form", "verify", "f1", "--file"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"valid\": true"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_field_is_made_from_flags_over_a_file() {
    let env = Env::new().await;
    let file = env.dir.path().join("field.json");
    std::fs::write(
        &file,
        r#"{"key": "old", "options": [{"value": "a", "labels": {"en": "A"}}]}"#,
    )
    .unwrap();
    env.expect(
        "POST",
        &tenant("/fields"),
        json!({
            "key": "member_no", "options": [{"value": "a", "labels": {"en": "A"}}],
            "type": "code", "labels": {"en": "Member"},
        }),
        json!({"id": "c1", "ref": "custom:member_no"}),
    )
    .await;
    env.cmd()
        .args(["field", "create", "--file"])
        .arg(&file)
        .args([
            "--key",
            "member_no",
            "--type",
            "code",
            "--label",
            "en=Member",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("custom:member_no"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_catalogue_needs_no_sign_in() {
    let env = Env::signed_out().await;
    let fields =
        json!({"fields": [{"id": "given_name", "labels": {"en": "Given name", "es": "Nombre"}}]});
    env.on("GET", "/api/v1/catalog/fields", fields).await;
    env.cmd()
        .args(["--locale", "es", "catalog", "fields"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nombre"));
    env.on(
        "GET",
        "/api/v1/catalog/issuers/iss_x/offers/enrollment",
        json!({"form": {}}),
    )
    .await;
    env.cmd()
        .args(["catalog", "offer", "iss_x", "enrollment"])
        .assert()
        .success();
}

// --- applications and issuance ----------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn applications_are_listed_by_issuer_and_decided() {
    let env = Env::new().await;
    Mock::given(method("GET"))
        .and(path(tenant("/applications")))
        .and(query_param("issuer_id", "i1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"id": "a1", "status": "submitted"}])),
        )
        .expect(1)
        .mount(&env.server)
        .await;
    env.cmd()
        .args(["application", "list", "--issuer", "i1"])
        .assert()
        .success();
    env.expect(
        "POST",
        &tenant("/applications/a1/decision"),
        json!({"decision": "rejected", "note": "Incomplete"}),
        json!({"id": "a1", "status": "rejected"}),
    )
    .await;
    env.cmd()
        .args([
            "application",
            "decide",
            "a1",
            "reject",
            "--note",
            "Incomplete",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_applications_file_is_saved_by_its_name_only() {
    let env = Env::new().await;
    Mock::given(method("GET"))
        .and(path(tenant("/applications/a1/files/id_scan")))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "content-disposition",
                    "attachment; filename=\"../scan.pdf\"",
                )
                .set_body_bytes(b"%PDF".to_vec()),
        )
        .mount(&env.server)
        .await;
    env.cmd()
        .current_dir(env.dir.path())
        .args(["application", "file", "a1", "id_scan"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read(env.dir.path().join("scan.pdf")).unwrap(),
        b"%PDF"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn issuance_settles_the_proposal_with_changes() {
    let env = Env::new().await;
    let proposal = json!({
        "claims": [
            {"field": {"id": "given_name"}, "value": "Grace"},
            {"field": {"id": "academic_year"}, "value": null},
        ],
        "valid_until": "2027-10-02",
    });
    env.on("GET", &tenant("/applications/a1/issuance"), proposal)
        .await;
    env.expect(
        "PUT",
        &tenant("/applications/a1/issuance"),
        json!({"claims": {"given_name": "Grace", "academic_year": "2026-2027"}, "valid_until": "2027-10-02"}),
        json!({"claims": {}, "valid_until": "2027-10-02"}),
    )
    .await;
    env.cmd()
        .args([
            "issuance",
            "set",
            "a1",
            "--claim",
            "academic_year=2026-2027",
        ])
        .assert()
        .success();
    env.wallet(&tenant("/applications/a1/issuance/sign"), "signed")
        .await;
    env.on(
        "GET",
        &tenant("/applications/a1"),
        json!({"id": "a1", "status": "issued"}),
    )
    .await;
    env.cmd()
        .args(["issuance", "sign", "a1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("issued"));
}

#[tokio::test(flavor = "multi_thread")]
async fn refusals_carry_the_apis_code_and_exit_status() {
    let env = Env::new().await;
    env.refuse(
        "GET",
        &tenant("/issuers/nope"),
        404,
        json!("issuer_not_found"),
    )
    .await;
    env.cmd()
        .args(["issuer", "get", "nope"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("issuer_not_found"));
    env.refuse(
        "POST",
        &tenant("/issuers"),
        422,
        json!([{"loc": ["body", "name"], "msg": "String should have at least 1 character"}]),
    )
    .await;
    env.cmd()
        .args(["issuer", "create", "--name", ""])
        .assert()
        .code(5)
        .stderr(predicate::str::contains(
            "name: String should have at least 1 character",
        ));
    env.refuse(
        "POST",
        &tenant("/identities/n1/sign"),
        403,
        json!("not_a_signer"),
    )
    .await;
    env.cmd()
        .args(["identity", "sign", "n1"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("link-wallet"));
}

#[tokio::test(flavor = "multi_thread")]
async fn json_output_is_the_apis_answer() {
    let env = Env::new().await;
    env.cmd()
        .args(["-o", "json", "tenant", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("\"id\": \"{TENANT}\"")));
}

// --- agent and config -------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn the_agent_answers_as_it_streams() {
    let env = Env::signed_out().await;
    let events = [
        json!({"result": {"task": {"id": "t", "contextId": "c1", "status": {"state": "TASK_STATE_SUBMITTED"}}}}),
        json!({"result": {"artifactUpdate": {"artifact": {"parts": [{"text": "Hello "}]}}}}),
        json!({"result": {"artifactUpdate": {"artifact": {"parts": [{"text": "there"}]}}}}),
        json!({"result": {"statusUpdate": {"contextId": "c1", "status": {"state": "TASK_STATE_COMPLETED"}}}}),
    ];
    let stream: String = events.iter().map(|e| format!("data: {e}\n\n")).collect();
    Mock::given(method("POST"))
        .and(path("/a2a/jsonrpc"))
        .and(header("a2a-version", "1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(stream, "text/event-stream"))
        .mount(&env.server)
        .await;
    env.cmd()
        .env("ALMENA_AGENT_URL", env.server.uri())
        .args(["agent", "ask", "Hi"])
        .assert()
        .success()
        .stdout("Hello there\n")
        .stderr(predicate::str::contains("--context c1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_declined_message_fails() {
    let env = Env::signed_out().await;
    let task =
        json!({"result": {"task": {"status": {"state": "TASK_STATE_REJECTED"}, "artifacts": []}}});
    env.on("POST", "/a2a/jsonrpc", task).await;
    env.cmd()
        .env("ALMENA_AGENT_URL", env.server.uri())
        .args(["agent", "ask", "--no-stream", "Hi"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("declined"));
}

#[tokio::test(flavor = "multi_thread")]
async fn settings_are_kept_per_profile() {
    let env = Env::signed_out().await;
    env.cmd()
        .args([
            "--profile",
            "staging",
            "config",
            "set",
            "api-url",
            "https://api.staging.test/",
        ])
        .assert()
        .success();
    env.cmd()
        .args(["--profile", "staging", "config", "set", "output", "json"])
        .assert()
        .success();
    env.cmd()
        .env_remove("ALMENA_API_URL")
        .args(["--profile", "staging", "config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"api_url\": \"https://api.staging.test\"",
        ));
    env.cmd()
        .args(["config", "set", "api-url", "ftp://nope"])
        .assert()
        .failure();
    env.cmd()
        .args(["--profile", "staging", "config", "unset", "output"])
        .assert()
        .success();
    let config = std::fs::read_to_string(env.dir.path().join("config.toml")).unwrap();
    assert!(config.contains("[profiles.staging]") && !config.contains("output"));
}
