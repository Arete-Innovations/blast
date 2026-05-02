use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use fantoccini::{ClientBuilder, Locator};
use serde_json::json;
use tokio::time::timeout;

const SERVER_URL: &str = "http://127.0.0.1:8000";
const DRIVER_PORT: u16 = 4445;
const PASSWORD: &str = "S3cure!Pass-word";
const SHOT_DIR: &str = "/tmp/shots";

#[tokio::main]
async fn main() -> Result<()> {
    std::fs::create_dir_all(SHOT_DIR)?;

    let mut driver = spawn_driver()?;
    let result = run().await;
    let _ = driver.kill();
    let _ = driver.wait();
    result
}

fn spawn_driver() -> Result<Child> {
    eprintln!("[shots] geckodriver port {DRIVER_PORT}");
    let child = Command::new("geckodriver")
        .args(["--port", &DRIVER_PORT.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn geckodriver")?;
    std::thread::sleep(Duration::from_millis(750));
    Ok(child)
}

async fn run() -> Result<()> {
    timeout(Duration::from_secs(60), wait_for_server())
        .await
        .map_err(|_| anyhow!("server never up"))??;
    eprintln!("[shots] server up");

    let mut caps = serde_json::Map::new();
    caps.insert(
        "moz:firefoxOptions".to_string(),
        json!({
            "args": ["-headless", "--width=1440", "--height=900"],
        }),
    );
    let driver_url = format!("http://127.0.0.1:{DRIVER_PORT}");
    let client = ClientBuilder::native()
        .capabilities(caps)
        .connect(&driver_url)
        .await
        .context("connect geckodriver")?;
    client
        .set_window_size(1440, 900)
        .await
        .context("resize window")?;

    shot_tall(&client, "/", "00_uikit_full").await?;
    shot_section(&client, "/", "tokens", "10_uikit_tokens").await?;
    shot_section(&client, "/", "buttons", "11_uikit_buttons").await?;
    shot_section(&client, "/", "forms", "12_uikit_forms").await?;
    shot_section(&client, "/", "feedback", "13_uikit_feedback").await?;
    shot_section(&client, "/", "layout", "14_uikit_layout").await?;
    shot_section(&client, "/", "cells", "15_uikit_cells").await?;
    shot_section(&client, "/", "dialogs", "16_uikit_dialogs").await?;
    shot(&client, "/?dialog=demo_confirm", "17_uikit_confirm").await?;
    shot(&client, "/?dialog=demo_drawer", "18_uikit_drawer").await?;
    shot(&client, "/", "01_welcome").await?;
    shot(&client, "/login", "20_login_styled").await?;
    shot(&client, "/register", "21_register_styled").await?;
    shot(&client, "/register", "02_register_empty").await?;

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let email = format!("shots+{suffix}@catablast.test");

    fill_register(&client, &email).await?;
    shot_at(&client, "03_register_filled").await?;

    let auth_email = format!("auth+{suffix}@catablast.test");
    register_via_api(&auth_email).await?;
    set_session_cookie(&client, &auth_email).await?;
    shot_tall(&client, "/dashboard", "30_dashboard_styled").await?;
    shot_tall(&client, "/profile", "31_profile_styled").await?;
    shot_tall(&client, "/admin", "32_admin_styled").await?;

    shot(&client, "/profile", "05_profile").await?;
    shot(&client, "/dashboard", "06_dashboard_cold").await?;

    logout(&email).await?;
    shot(&client, "/login", "07_login_empty").await?;
    fill_login(&client, &email).await?;
    shot_at(&client, "08_login_filled").await?;

    shot(&client, "/nonexistent-route", "09_not_found").await?;

    let _ = client.close().await;
    eprintln!("[shots] done — {SHOT_DIR}/");
    Ok(())
}

async fn wait_for_server() -> Result<()> {
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;
    loop {
        if let Ok(r) = c.get(format!("{SERVER_URL}/api/healthz")).send().await {
            if r.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn shot(client: &fantoccini::Client, path: &str, name: &str) -> Result<()> {
    eprintln!("[shots] GET {path}");
    client.goto(&format!("{SERVER_URL}{path}")).await?;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    shot_at(client, name).await
}

async fn shot_section(client: &fantoccini::Client, path: &str, anchor: &str, name: &str) -> Result<()> {
    eprintln!("[shots] section #{anchor}");
    client.goto(&format!("{SERVER_URL}{path}#{anchor}")).await?;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    client
        .execute(
            &format!("document.getElementById('{anchor}').scrollIntoView({{block:'start'}});return null;"),
            vec![],
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(400)).await;
    shot_at(client, name).await
}

async fn shot_tall(client: &fantoccini::Client, path: &str, name: &str) -> Result<()> {
    eprintln!("[shots] tall GET {path}");
    client.goto(&format!("{SERVER_URL}{path}")).await?;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let height = client
        .execute("return Math.max(document.body.scrollHeight, document.documentElement.scrollHeight)", vec![])
        .await?
        .as_u64()
        .unwrap_or(2400);
    let h = height.clamp(900, 12000) as u32;
    client.set_window_size(1440, h).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    shot_at(client, name).await?;
    client.set_window_size(1440, 900).await?;
    Ok(())
}

async fn shot_at(client: &fantoccini::Client, name: &str) -> Result<()> {
    let bytes = client.screenshot().await?;
    let p = format!("{SHOT_DIR}/{name}.png");
    std::fs::write(&p, bytes)?;
    eprintln!("[shots] wrote {p}");
    Ok(())
}

async fn fill_register(client: &fantoccini::Client, email: &str) -> Result<()> {
    client.wait().for_element(Locator::Css("form")).await?;
    client
        .find(Locator::Css("input[type=email]"))
        .await?
        .send_keys(email)
        .await?;
    let pws = client
        .find_all(Locator::Css("input[type=password]"))
        .await?;
    if pws.len() < 2 {
        bail!("expected 2 password fields");
    }
    pws[0].clone().send_keys(PASSWORD).await?;
    pws[1].clone().send_keys(PASSWORD).await?;
    Ok(())
}

async fn fill_login(client: &fantoccini::Client, email: &str) -> Result<()> {
    client.wait().for_element(Locator::Css("form")).await?;
    client
        .find(Locator::Css("input[type=email]"))
        .await?
        .send_keys(email)
        .await?;
    client
        .find(Locator::Css("input[type=password]"))
        .await?
        .send_keys(PASSWORD)
        .await?;
    Ok(())
}

async fn submit_and_settle(client: &fantoccini::Client) -> Result<()> {
    client
        .execute(
            "setTimeout(()=>document.querySelector('button[type=submit]').click(),50);return null;",
            vec![],
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    Ok(())
}

async fn register_via_api(email: &str) -> Result<()> {
    let c = reqwest::Client::builder().timeout(Duration::from_secs(5)).build()?;
    let r = c
        .post(format!("{SERVER_URL}/api/auth/register"))
        .json(&json!({"email": email, "password": PASSWORD}))
        .send()
        .await?;
    if !r.status().is_success() {
        let s = r.status();
        let body = r.text().await.unwrap_or_default();
        bail!("api register failed: {} body={}", s, body);
    }
    Ok(())
}

async fn set_session_cookie(client: &fantoccini::Client, email: &str) -> Result<()> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let c = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()?;
    let r = c
        .post(format!("{SERVER_URL}/api/auth/login"))
        .json(&json!({"email": email, "password": PASSWORD}))
        .send()
        .await?;
    if !r.status().is_success() {
        let s = r.status();
        let body = r.text().await.unwrap_or_default();
        bail!("api login (cookie pickup) failed: {} body={}", s, body);
    }
    let set_cookie_headers: Vec<_> = r
        .headers()
        .get_all("set-cookie")
        .into_iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect();
    if set_cookie_headers.is_empty() {
        bail!("api login returned no set-cookie header");
    }
    client.goto(SERVER_URL).await?;
    for raw in &set_cookie_headers {
        if let Some(cookie_pair) = raw.split(';').next() {
            if let Some((name, value)) = cookie_pair.split_once('=') {
                client
                    .execute(
                        &format!("document.cookie = '{}={};path=/';return null;", name.trim(), value.trim()),
                        vec![],
                    )
                    .await?;
            }
        }
    }
    Ok(())
}

async fn logout(email: &str) -> Result<()> {
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let c = reqwest::Client::builder()
        .cookie_provider(jar)
        .timeout(Duration::from_secs(5))
        .build()?;
    let r = c
        .post(format!("{SERVER_URL}/api/auth/login"))
        .json(&json!({"email": email, "password": PASSWORD}))
        .send()
        .await?;
    if !r.status().is_success() {
        bail!("api login (for logout) failed: {}", r.status());
    }
    let r = c
        .post(format!("{SERVER_URL}/api/auth/logout"))
        .send()
        .await?;
    if !r.status().is_success() {
        bail!("api logout failed: {}", r.status());
    }
    Ok(())
}
