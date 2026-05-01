use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use fantoccini::{ClientBuilder, Locator};
use serde_json::json;

const SERVER_URL: &str = "http://127.0.0.1:8000";
const CHROMEDRIVER_PORT: u16 = 9515;

#[tokio::main]
async fn main() -> Result<()> {
    let mut driver = spawn_chromedriver()?;
    let outcome = run_suite().await;
    let _ = driver.kill();
    let _ = driver.wait();
    outcome
}

fn spawn_chromedriver() -> Result<Child> {
    let child = Command::new("chromedriver")
        .arg(format!("--port={}", CHROMEDRIVER_PORT))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn chromedriver — is it on PATH? install via pacman -S chromedriver")?;
    std::thread::sleep(Duration::from_millis(500));
    Ok(child)
}

async fn run_suite() -> Result<()> {
    wait_for_server().await?;

    let caps = serde_json::Map::from_iter([(
        "goog:chromeOptions".to_string(),
        json!({ "args": ["--headless=new", "--no-sandbox", "--disable-dev-shm-usage"] }),
    )]);

    let driver_url = format!("http://127.0.0.1:{}", CHROMEDRIVER_PORT);
    let client = ClientBuilder::native()
        .capabilities(caps)
        .connect(&driver_url)
        .await
        .context("connect to chromedriver")?;

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let email = format!("e2e+{}@catablast.test", suffix);
    let password = "S3cure!Pass-word";

    register_via_ui(&client, &email, password).await?;
    println!("[e2e] register_via_ui OK ({})", email);

    logout_via_api(&email).await?;
    println!("[e2e] logout_via_api OK");

    login_via_ui(&client, &email, password).await?;
    println!("[e2e] login_via_ui OK");

    auth_me_via_api(&email).await?;
    println!("[e2e] auth_me_via_api OK");

    client.close().await?;
    Ok(())
}

async fn wait_for_server() -> Result<()> {
    let client = reqwest::Client::new();
    for attempt in 0..60 {
        match client.get(format!("{}/api/healthz", SERVER_URL)).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if attempt % 10 == 0 && attempt > 0 {
                    eprintln!("[e2e] still waiting for {}/api/healthz...", SERVER_URL);
                }
            }
        }
    }
    Err(anyhow!("server did not become ready at {}", SERVER_URL))
}

async fn register_via_ui(client: &fantoccini::Client, email: &str, password: &str) -> Result<()> {
    client.goto(&format!("{}/register", SERVER_URL)).await?;
    client.wait().for_element(Locator::Css("form")).await?;
    let form = client.form(Locator::Css("form")).await?;
    form.set(Locator::Css("input[type=email]"), email).await?;
    let pw_inputs = client.find_all(Locator::Css("input[type=password]")).await?;
    if pw_inputs.len() < 2 {
        return Err(anyhow!("expected 2 password inputs on register, got {}", pw_inputs.len()));
    }
    pw_inputs[0].clone().send_keys(password).await?;
    pw_inputs[1].clone().send_keys(password).await?;
    client.find(Locator::Css("button[type=submit]")).await?.click().await?;
    client
        .wait()
        .at_most(Duration::from_secs(10))
        .for_url(reqwest::Url::parse(&format!("{}/dashboard", SERVER_URL))?)
        .await?;
    Ok(())
}

async fn login_via_ui(client: &fantoccini::Client, email: &str, password: &str) -> Result<()> {
    client.goto(&format!("{}/login", SERVER_URL)).await?;
    client.wait().for_element(Locator::Css("form")).await?;
    let form = client.form(Locator::Css("form")).await?;
    form.set(Locator::Css("input[type=email]"), email).await?;
    form.set(Locator::Css("input[type=password]"), password).await?;
    client.find(Locator::Css("button[type=submit]")).await?.click().await?;
    client
        .wait()
        .at_most(Duration::from_secs(10))
        .for_url(reqwest::Url::parse(&format!("{}/dashboard", SERVER_URL))?)
        .await?;
    Ok(())
}

async fn logout_via_api(email: &str) -> Result<()> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .build()?;

    let login_resp = client
        .post(format!("{}/api/auth/login", SERVER_URL))
        .json(&json!({ "email": email, "password": "S3cure!Pass-word" }))
        .send()
        .await?;
    if !login_resp.status().is_success() {
        return Err(anyhow!("api login for logout setup failed: {}", login_resp.status()));
    }

    let logout_resp = client.post(format!("{}/api/auth/logout", SERVER_URL)).send().await?;
    if !logout_resp.status().is_success() {
        return Err(anyhow!("api logout failed: {}", logout_resp.status()));
    }
    Ok(())
}

async fn auth_me_via_api(email: &str) -> Result<()> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let client = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .build()?;

    let login_resp = client
        .post(format!("{}/api/auth/login", SERVER_URL))
        .json(&json!({ "email": email, "password": "S3cure!Pass-word" }))
        .send()
        .await?;
    if !login_resp.status().is_success() {
        return Err(anyhow!("api login failed: {}", login_resp.status()));
    }

    let me_resp = client.get(format!("{}/api/auth/me", SERVER_URL)).send().await?;
    if !me_resp.status().is_success() {
        return Err(anyhow!("api /me failed: {}", me_resp.status()));
    }
    let me: serde_json::Value = me_resp.json().await?;
    let returned_email = me.get("email").and_then(|v| v.as_str()).unwrap_or("");
    if returned_email != email {
        return Err(anyhow!("auth_me email mismatch: got {}, expected {}", returned_email, email));
    }
    Ok(())
}
