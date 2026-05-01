use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use fantoccini::{ClientBuilder, Locator};
use serde_json::json;
use tokio::time::timeout;

const SERVER_URL: &str = "http://127.0.0.1:8000";
const DRIVER_PORT: u16 = 4444;
const STEP_TIMEOUT: Duration = Duration::from_secs(40);
const SERVER_BOOT_TIMEOUT: Duration = Duration::from_secs(60);
const PASSWORD: &str = "S3cure!Pass-word";

// Substrings that, when seen in any captured console-error / uncaught / unhandled-promise
// entry, MUST fail the suite. These are the fingerprints of tachys hydration panics and
// console_error_panic_hook dumps. Keep this list narrow so leptos dev-server warnings
// don't false-positive.
const FATAL_CONSOLE_SUBSTRINGS: &[&str] = &[
    "hydration",
    "panicked",
    "unreachable executed",
    "Unrecoverable",
    "expected a marker node",
    "RuntimeError",
    "wasm-bindgen",
];

#[tokio::main]
async fn main() -> Result<()> {
    let mut driver = spawn_driver()?;

    let result = run_with_master_timeout().await;

    eprintln!("[e2e] killing geckodriver");
    let _ = driver.kill();
    let _ = driver.wait();

    result
}

async fn run_with_master_timeout() -> Result<()> {
    match timeout(Duration::from_secs(180), run_suite()).await {
        Ok(r) => r,
        Err(_elapsed) => bail!("master timeout (180s) — suite hung"),
    }
}

fn spawn_driver() -> Result<Child> {
    eprintln!("[e2e] spawning geckodriver on port {}", DRIVER_PORT);
    let child = Command::new("geckodriver")
        .args(["--port", &DRIVER_PORT.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn geckodriver — install via pacman -S geckodriver")?;
    std::thread::sleep(Duration::from_millis(750));
    Ok(child)
}

async fn run_suite() -> Result<()> {
    eprintln!("[e2e] waiting for server at {}/api/healthz", SERVER_URL);
    timeout(SERVER_BOOT_TIMEOUT, wait_for_server())
        .await
        .map_err(|_| anyhow!("server never responded on /api/healthz within {:?}", SERVER_BOOT_TIMEOUT))??;
    eprintln!("[e2e] server up");

    let mut caps = serde_json::Map::new();
    caps.insert(
        "moz:firefoxOptions".to_string(),
        json!({
            "args": ["-headless"],
            "prefs": {
                "dom.disable_beforeunload": true,
                "dom.webnotifications.enabled": false,
            }
        }),
    );

    let driver_url = format!("http://127.0.0.1:{}", DRIVER_PORT);
    let client = ClientBuilder::native()
        .capabilities(caps)
        .connect(&driver_url)
        .await
        .context("connect to geckodriver — is firefox installed?")?;
    eprintln!("[e2e] geckodriver session ready");

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let email = format!("e2e+{}@catablast.test", suffix);

    timeout(STEP_TIMEOUT, smoke_welcome(&client))
        .await
        .map_err(|_| anyhow!("smoke_welcome timed out after {:?}", STEP_TIMEOUT))??;
    eprintln!("[e2e] ✓ smoke_welcome");

    timeout(STEP_TIMEOUT, register_via_ui(&client, &email))
        .await
        .map_err(|_| anyhow!("register_via_ui timed out after {:?}", STEP_TIMEOUT))??;
    eprintln!("[e2e] ✓ register_via_ui ({})", email);

    timeout(STEP_TIMEOUT, logout_via_api(&email))
        .await
        .map_err(|_| anyhow!("logout_via_api timed out after {:?}", STEP_TIMEOUT))??;
    eprintln!("[e2e] ✓ logout_via_api");

    timeout(STEP_TIMEOUT, login_via_ui(&client, &email))
        .await
        .map_err(|_| anyhow!("login_via_ui timed out after {:?}", STEP_TIMEOUT))??;
    eprintln!("[e2e] ✓ login_via_ui");

    timeout(STEP_TIMEOUT, cold_dashboard_after_login(&client))
        .await
        .map_err(|_| anyhow!("cold_dashboard_after_login timed out after {:?}", STEP_TIMEOUT))??;
    eprintln!("[e2e] ✓ cold_dashboard_after_login");

    timeout(STEP_TIMEOUT, cold_login_redirects_when_authed(&client))
        .await
        .map_err(|_| anyhow!("cold_login_redirects_when_authed timed out after {:?}", STEP_TIMEOUT))??;
    eprintln!("[e2e] ✓ cold_login_redirects_when_authed");

    timeout(STEP_TIMEOUT, auth_me_via_api(&email))
        .await
        .map_err(|_| anyhow!("auth_me_via_api timed out after {:?}", STEP_TIMEOUT))??;
    eprintln!("[e2e] ✓ auth_me_via_api");

    let _ = client.close().await;
    eprintln!("[e2e] all steps passed");
    Ok(())
}

async fn wait_for_server() -> Result<()> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build()?;
    loop {
        match client.get(format!("{}/api/healthz", SERVER_URL)).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
}

// Install the console-capture hook into the current page. Idempotent per page-load
// (the hook self-guards via `window.__e2e_console_hooked`); navigating to a new URL
// blows the global away and we re-install on the next call. Also resets the queue
// so each step asserts against its own slice.
async fn install_console_hook(client: &fantoccini::Client) -> Result<()> {
    let install_script = r#"
        if (!window.__e2e_console_hooked) {
            window.__e2e_console_hooked = true;
            window.__e2e_console = [];
            const orig = { log: console.log, warn: console.warn, error: console.error };
            console.log = function() { window.__e2e_console.push(['log', Array.from(arguments).map(String).join(' ')]); orig.log.apply(console, arguments); };
            console.warn = function() { window.__e2e_console.push(['warn', Array.from(arguments).map(String).join(' ')]); orig.warn.apply(console, arguments); };
            console.error = function() { window.__e2e_console.push(['error', Array.from(arguments).map(String).join(' ')]); orig.error.apply(console, arguments); };
            window.addEventListener('error', (e) => window.__e2e_console.push(['uncaught', String(e.message) + ' @ ' + String(e.filename) + ':' + String(e.lineno)]));
            window.addEventListener('unhandledrejection', (e) => window.__e2e_console.push(['unhandled-promise', String(e.reason)]));
        }
        return null;
    "#;
    client.execute(install_script, vec![]).await?;
    Ok(())
}

// Drain captured console entries and panic the test if any look like a hydration
// failure / wasm panic. Returns the (kind, message) pairs so the caller can log
// the full diagnostic on failure.
async fn assert_no_fatal_console(client: &fantoccini::Client, label: &str) -> Result<()> {
    let raw = client
        .execute("return JSON.stringify(window.__e2e_console || [])", vec![])
        .await?;
    let entries: Vec<(String, String)> = match raw.as_str() {
        Some(s) => serde_json::from_str(s).unwrap_or_default(),
        None => Vec::new(),
    };

    let fatal: Vec<&(String, String)> = entries
        .iter()
        .filter(|(kind, msg)| {
            // Only error-level kinds count toward fatality. Leptos dev-server can
            // emit warn-level noise (HMR, vite probes) that we don't want to fail on.
            let is_error_kind = matches!(
                kind.as_str(),
                "error" | "uncaught" | "unhandled-promise"
            );
            if !is_error_kind {
                return false;
            }
            FATAL_CONSOLE_SUBSTRINGS
                .iter()
                .any(|needle| msg.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()))
        })
        .collect();

    if !fatal.is_empty() {
        eprintln!("[e2e]   --- fatal console entries ({}) ---", label);
        for (kind, msg) in &fatal {
            eprintln!("[e2e]   [{}] {}", kind, msg);
        }
        eprintln!("[e2e]   --- full console dump ---");
        for (kind, msg) in &entries {
            eprintln!("[e2e]   [{}] {}", kind, msg);
        }
        dump_diagnostics(client, label).await;
        bail!(
            "{}: {} fatal console entries (hydration / wasm panic fingerprint)",
            label,
            fatal.len()
        );
    }
    Ok(())
}

// Reset the captured-error queue. Use after a navigation if we want fresh-page-only
// assertions for the next step.
async fn reset_console(client: &fantoccini::Client) -> Result<()> {
    client
        .execute("window.__e2e_console = []; return null;", vec![])
        .await?;
    Ok(())
}

async fn smoke_welcome(client: &fantoccini::Client) -> Result<()> {
    eprintln!("[e2e]   smoke: GET {}/", SERVER_URL);
    client.goto(SERVER_URL).await?;
    eprintln!("[e2e]   smoke: install console hook");
    install_console_hook(client).await?;
    wait_for_hydration(client, Duration::from_secs(3)).await?;
    eprintln!("[e2e]   smoke: probing browser via execute()");
    match tokio::time::timeout(Duration::from_secs(5), client.execute("return 1+1", vec![])).await {
        Ok(Ok(v)) => eprintln!("[e2e]   smoke: execute returned {}", v),
        Ok(Err(err)) => bail!("execute errored: {}", err),
        Err(_elapsed) => {
            eprintln!("[e2e]   smoke: execute hung 5s — wasm hydrate likely deadlocked main thread");
            dump_diagnostics(client, "smoke-hung").await;
            bail!("wasm hydrate hung the JS event loop on welcome page");
        }
    }
    assert_no_fatal_console(client, "smoke_welcome").await?;
    Ok(())
}

async fn register_via_ui(client: &fantoccini::Client, email: &str) -> Result<()> {
    eprintln!("[e2e]   register: GET {}/register", SERVER_URL);
    client.goto(&format!("{}/register", SERVER_URL)).await?;
    install_console_hook(client).await?;
    eprintln!("[e2e]   register: waiting for form");
    client.wait().at_most(Duration::from_secs(10)).for_element(Locator::Css("form")).await?;

    wait_for_hydration(client, Duration::from_secs(5)).await?;

    eprintln!("[e2e]   register: filling email");
    let email_input = client.find(Locator::Css("input[type=email]")).await?;
    email_input.send_keys(email).await?;

    eprintln!("[e2e]   register: filling passwords");
    let pw_inputs = client.find_all(Locator::Css("input[type=password]")).await?;
    if pw_inputs.len() < 2 {
        bail!("expected 2 password inputs on register, got {}", pw_inputs.len());
    }
    pw_inputs[0].clone().send_keys(PASSWORD).await?;
    pw_inputs[1].clone().send_keys(PASSWORD).await?;

    eprintln!("[e2e]   register: hooking fetch (uses Reflect.apply with bound this=window)");
    client
        .execute(
            r#"
            window.__e2e_fetches = [];
            const origFetch = window.fetch.bind(window);
            window.fetch = function() {
                const args = Array.from(arguments);
                let url = '';
                try { url = typeof args[0] === 'string' ? args[0] : (args[0] && args[0].url) || String(args[0]); } catch (e) { url = '<err>'; }
                window.__e2e_fetches.push({ url: url, t: Date.now() });
                return origFetch.apply(null, args);
            };
            return null;
        "#,
            vec![],
        )
        .await?;

    eprintln!("[e2e]   register: scheduling click via setTimeout (so execute returns immediately)");
    client
        .execute(
            "setTimeout(() => document.querySelector('button[type=submit]').click(), 50); return null;",
            vec![],
        )
        .await?;

    eprintln!("[e2e]   register: poll thread liveness for 8s");
    let mut last_alive = std::time::Instant::now();
    let mut alive_ticks = 0;
    let probe_deadline = std::time::Instant::now() + Duration::from_secs(8);
    while std::time::Instant::now() < probe_deadline {
        match tokio::time::timeout(Duration::from_secs(2), client.execute("return Date.now()", vec![])).await {
            Ok(Ok(_v)) => {
                last_alive = std::time::Instant::now();
                alive_ticks += 1;
            }
            Ok(Err(err)) => {
                eprintln!("[e2e]   register: execute err during poll: {}", err);
                break;
            }
            Err(_) => {
                eprintln!("[e2e]   register: JS thread blocked >2s (alive_ticks before block: {})", alive_ticks);
                dump_diagnostics(client, "register-deadlock").await;
                bail!("wasm wedged the JS thread");
            }
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    eprintln!(
        "[e2e]   register: js alive {} ticks, last alive {:?} ago",
        alive_ticks,
        last_alive.elapsed()
    );

    eprintln!("[e2e]   register: dumping fetches + console");
    if let Ok(v) = client.execute("return JSON.stringify(window.__e2e_fetches || [])", vec![]).await {
        eprintln!("[e2e]   register: fetches = {}", v);
    }
    if let Ok(v) = client.execute("return JSON.stringify(window.__e2e_console || [])", vec![]).await {
        eprintln!("[e2e]   register: console = {}", v);
    }

    eprintln!("[e2e]   register: awaiting /dashboard");
    if let Err(err) = wait_for_path(client, "/dashboard", Duration::from_secs(10)).await {
        dump_diagnostics(client, "register-failure").await;
        return Err(err);
    }
    assert_no_fatal_console(client, "register_via_ui").await?;
    Ok(())
}

async fn wait_for_hydration(client: &fantoccini::Client, deadline: Duration) -> Result<()> {
    install_console_hook(client).await?;
    tokio::time::sleep(deadline).await;
    Ok(())
}

async fn dump_diagnostics(client: &fantoccini::Client, label: &str) {
    eprintln!("[e2e]   --- diagnostics ({}) ---", label);
    match client.current_url().await {
        Ok(url) => eprintln!("[e2e]   url: {}", url),
        Err(err) => eprintln!("[e2e]   url: <err {}>", err),
    }
    match client.execute("return JSON.stringify(window.__e2e_console || [])", vec![]).await {
        Ok(value) => eprintln!("[e2e]   console: {}", value),
        Err(err) => eprintln!("[e2e]   console: <err {}>", err),
    }
    match client.execute("return document && document.body ? String(document.body.innerHTML.length) : '-1'", vec![]).await {
        Ok(value) => eprintln!("[e2e]   body bytes: {}", value),
        Err(err) => eprintln!("[e2e]   body bytes: <err {}>", err),
    }
    match client.source().await {
        Ok(html) => {
            let snippet: String = html.chars().take(800).collect();
            eprintln!("[e2e]   page source head (800 chars): {}", snippet);
        }
        Err(err) => eprintln!("[e2e]   page source: <err {}>", err),
    }
}

async fn login_via_ui(client: &fantoccini::Client, email: &str) -> Result<()> {
    eprintln!("[e2e]   login: GET {}/login", SERVER_URL);
    client.goto(&format!("{}/login", SERVER_URL)).await?;
    install_console_hook(client).await?;
    eprintln!("[e2e]   login: waiting for form");
    client.wait().at_most(Duration::from_secs(10)).for_element(Locator::Css("form")).await?;

    wait_for_hydration(client, Duration::from_secs(3)).await?;

    eprintln!("[e2e]   login: filling email");
    client.find(Locator::Css("input[type=email]")).await?.send_keys(email).await?;
    eprintln!("[e2e]   login: filling password");
    client.find(Locator::Css("input[type=password]")).await?.send_keys(PASSWORD).await?;

    client
        .execute(
            r#"
            window.__e2e_fetches = [];
            const origFetch = window.fetch.bind(window);
            window.fetch = function() {
                const args = Array.from(arguments);
                let url = '';
                try { url = typeof args[0] === 'string' ? args[0] : (args[0] && args[0].url) || String(args[0]); } catch (e) { url = '<err>'; }
                window.__e2e_fetches.push({ url: url, t: Date.now() });
                return origFetch.apply(null, args);
            };
            return null;
        "#,
            vec![],
        )
        .await?;

    eprintln!("[e2e]   login: scheduling click via setTimeout");
    client
        .execute("setTimeout(() => document.querySelector('button[type=submit]').click(), 50); return null;", vec![])
        .await?;

    eprintln!("[e2e]   login: poll thread liveness 6s");
    let mut alive_ticks = 0;
    let probe_deadline = std::time::Instant::now() + Duration::from_secs(6);
    while std::time::Instant::now() < probe_deadline {
        match tokio::time::timeout(Duration::from_secs(2), client.execute("return Date.now()", vec![])).await {
            Ok(Ok(_v)) => alive_ticks += 1,
            Ok(Err(err)) => {
                eprintln!("[e2e]   login: execute err: {}", err);
                break;
            }
            Err(_) => {
                eprintln!("[e2e]   login: JS thread blocked >2s ({} ticks)", alive_ticks);
                dump_diagnostics(client, "login-deadlock").await;
                bail!("login wedged JS thread");
            }
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    eprintln!("[e2e]   login: alive {} ticks", alive_ticks);

    if let Ok(v) = client.execute("return JSON.stringify(window.__e2e_fetches || [])", vec![]).await {
        eprintln!("[e2e]   login: fetches = {}", v);
    }
    if let Ok(v) = client.execute("return JSON.stringify(window.__e2e_console || [])", vec![]).await {
        eprintln!("[e2e]   login: console = {}", v);
    }

    eprintln!("[e2e]   login: awaiting /dashboard");
    if let Err(err) = wait_for_path(client, "/dashboard", Duration::from_secs(10)).await {
        dump_diagnostics(client, "login-failure").await;
        return Err(err);
    }
    assert_no_fatal_console(client, "login_via_ui").await?;
    Ok(())
}

// Cold-load the protected dashboard route via full SSR navigation while authed.
// This is the regression net for the tachys "expected a marker node" hydration
// panic: soft-nav inside the SPA router uses CSR, which never exercises the
// SSR-then-hydrate path. A `client.goto()` to an absolute URL forces firefox to
// fetch the SSR'd HTML and lets the wasm bundle attempt to hydrate against it.
async fn cold_dashboard_after_login(client: &fantoccini::Client) -> Result<()> {
    eprintln!("[e2e]   cold-dashboard: full-page goto {}/dashboard", SERVER_URL);
    client.goto(&format!("{}/dashboard", SERVER_URL)).await?;
    install_console_hook(client).await?;
    reset_console(client).await?;

    eprintln!("[e2e]   cold-dashboard: waiting for body");
    client
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css("body"))
        .await?;

    // Give wasm time to download + hydrate. Tachys hydration mismatches surface
    // as console.error (via console_error_panic_hook) within the first few hundred
    // ms after wasm boots. 1500ms is a comfortable upper bound.
    tokio::time::sleep(Duration::from_millis(1500)).await;

    eprintln!("[e2e]   cold-dashboard: asserting URL still /dashboard (not bounced to /login)");
    let url = client.current_url().await?;
    if url.path() != "/dashboard" {
        dump_diagnostics(client, "cold-dashboard-wrong-url").await;
        bail!(
            "cold-dashboard navigation bounced to {} — auth cookie not honored or hydration error redirected",
            url.path()
        );
    }

    eprintln!("[e2e]   cold-dashboard: asserting page-shell heading present");
    let h1 = client
        .wait()
        .at_most(Duration::from_secs(5))
        .for_element(Locator::Css("h1"))
        .await
        .context("cold-dashboard: no <h1> rendered after hydration")?;
    let heading_text = h1.text().await?;
    if !heading_text.contains("Dashboard") {
        dump_diagnostics(client, "cold-dashboard-bad-heading").await;
        bail!("cold-dashboard: expected <h1>Dashboard</h1>, got {:?}", heading_text);
    }

    assert_no_fatal_console(client, "cold_dashboard_after_login").await?;
    Ok(())
}

// Cold-load /login while authed. AnonOnly should bounce us to /dashboard. This
// will fail until the structural AnonOnly fix lands — that's intentional.
async fn cold_login_redirects_when_authed(client: &fantoccini::Client) -> Result<()> {
    eprintln!("[e2e]   cold-login-redirect: full-page goto {}/login (while authed)", SERVER_URL);
    client.goto(&format!("{}/login", SERVER_URL)).await?;
    install_console_hook(client).await?;
    reset_console(client).await?;

    eprintln!("[e2e]   cold-login-redirect: waiting for body");
    client
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css("body"))
        .await?;

    tokio::time::sleep(Duration::from_millis(1500)).await;

    eprintln!("[e2e]   cold-login-redirect: asserting URL flipped to /dashboard");
    if let Err(err) = wait_for_path(client, "/dashboard", Duration::from_secs(5)).await {
        dump_diagnostics(client, "cold-login-redirect-failure").await;
        return Err(err);
    }

    assert_no_fatal_console(client, "cold_login_redirects_when_authed").await?;
    Ok(())
}

async fn wait_for_path(client: &fantoccini::Client, expected_path: &str, deadline: Duration) -> Result<()> {
    let start = std::time::Instant::now();
    loop {
        if start.elapsed() > deadline {
            let url = client.current_url().await.map(|u| u.to_string()).unwrap_or_else(|_| "<err>".to_string());
            bail!("expected url to contain {} within {:?}, got {}", expected_path, deadline, url);
        }
        let url = client.current_url().await?;
        if url.path() == expected_path {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn logout_via_api(email: &str) -> Result<()> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .timeout(Duration::from_secs(5))
        .build()?;

    let login_resp = client
        .post(format!("{}/api/auth/login", SERVER_URL))
        .json(&json!({ "email": email, "password": PASSWORD }))
        .send()
        .await?;
    if !login_resp.status().is_success() {
        let status = login_resp.status();
        let body = login_resp.text().await.unwrap_or_default();
        bail!("api login for logout setup failed: {} body={}", status, body);
    }

    let logout_resp = client.post(format!("{}/api/auth/logout", SERVER_URL)).send().await?;
    if !logout_resp.status().is_success() {
        bail!("api logout failed: {}", logout_resp.status());
    }
    Ok(())
}

async fn auth_me_via_api(email: &str) -> Result<()> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .timeout(Duration::from_secs(5))
        .build()?;

    let login_resp = client
        .post(format!("{}/api/auth/login", SERVER_URL))
        .json(&json!({ "email": email, "password": PASSWORD }))
        .send()
        .await?;
    if !login_resp.status().is_success() {
        let status = login_resp.status();
        let body = login_resp.text().await.unwrap_or_default();
        bail!("api login failed: {} body={}", status, body);
    }
    let login_body: serde_json::Value = login_resp.json().await?;
    let session_id_at_login = login_body.get("session_id").and_then(|v| v.as_i64()).unwrap_or(-1);
    let user_id_at_login = login_body.get("user_id").and_then(|v| v.as_i64()).unwrap_or(-1);
    if session_id_at_login < 0 || user_id_at_login < 0 {
        bail!("api login response missing session_id/user_id: {}", login_body);
    }

    let me_resp = client.get(format!("{}/api/auth/me", SERVER_URL)).send().await?;
    if !me_resp.status().is_success() {
        bail!("api /me failed: {}", me_resp.status());
    }
    let me: serde_json::Value = me_resp.json().await?;
    let me_user_id = me.get("user_id").and_then(|v| v.as_i64()).unwrap_or(-1);
    if me_user_id != user_id_at_login {
        bail!("auth_me user_id mismatch: got {}, expected {} (email {})", me_user_id, user_id_at_login, email);
    }
    Ok(())
}
