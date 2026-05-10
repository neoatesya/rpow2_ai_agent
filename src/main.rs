use chrono::Local;
use colored::*;
use num_format::{Locale, ToFormattedString};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

const API_BASE: &str = "https://api.rpow2.com";
const BASE_UNITS_PER_RPOW: f64 = 1_000_000_000.0; // 9 decimals
const SESSION_FILE: &str = ".rpow-session.json";

// ─── Data Structures ─────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug)]
struct SessionData {
    email: String,
    cookies: Vec<String>,
}

#[derive(Serialize)]
struct AuthRequest {
    email: String,
}

#[derive(Deserialize, Debug)]
struct ChallengeResponse {
    challenge_id: String,
    nonce_prefix: String,
    difficulty_bits: u32,
    #[allow(dead_code)]
    expires_at: String,
}

#[derive(Serialize)]
struct MintRequest<'a> {
    challenge_id: &'a str,
    solution_nonce: String,
}

#[derive(Deserialize, Debug)]
struct MintResponse {
    token: Option<MintToken>,
    error: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CooldownResponse {
    #[allow(dead_code)]
    error: Option<String>,
    #[allow(dead_code)]
    message: Option<String>,
    retry_after: Option<u64>,
}

#[derive(Deserialize, Debug)]
struct MintToken {
    id: String,
    #[serde(default)]
    value_base_units: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    issued_at: Option<String>,
}

#[derive(Deserialize, Debug)]
struct MeResponse {
    #[allow(dead_code)]
    email: Option<String>,
    balance_base_units: Option<String>,
    #[allow(dead_code)]
    minted_base_units: Option<String>,
    #[allow(dead_code)]
    daily_mint_cap_base_units: Option<String>,
    #[allow(dead_code)]
    daily_minted_base_units: Option<String>,
    daily_remaining_base_units: Option<String>,
}

// ─── Mining Engine ───────────────────────────────────────────

fn count_trailing_zero_bits(hash: &[u8]) -> u32 {
    let mut count = 0;
    for &byte in hash.iter().rev() {
        if byte == 0 {
            count += 8;
        } else {
            count += byte.trailing_zeros();
            break;
        }
    }
    count
}

fn mine_challenge(nonce_prefix_hex: &str, difficulty_bits: u32, cores: usize) -> (u64, f64) {
    let nonce_prefix = hex::decode(nonce_prefix_hex).expect("Invalid nonce prefix hex");
    let found = Arc::new(AtomicBool::new(false));
    let start_time = Instant::now();

    let mut handles = vec![];
    for thread_id in 0..cores as u64 {
        let prefix = nonce_prefix.clone();
        let found = Arc::clone(&found);
        handles.push(thread::spawn(move || {
            let mut nonce = thread_id;
            let step = cores as u64;

            let mut base_hasher = Sha256::new();
            base_hasher.update(&prefix);

            loop {
                if found.load(Ordering::Relaxed) {
                    return None;
                }

                for _ in 0..4096 {
                    let mut hasher = base_hasher.clone();
                    hasher.update(&nonce.to_le_bytes());
                    let result = hasher.finalize();

                    if count_trailing_zero_bits(&result) >= difficulty_bits {
                        found.store(true, Ordering::Relaxed);
                        return Some(nonce);
                    }
                    nonce += step;
                }
            }
        }));
    }

    let mut solution = 0;
    for handle in handles {
        if let Some(s) = handle.join().unwrap() {
            solution = s;
        }
    }

    let duration = start_time.elapsed().as_secs_f64();
    (solution, duration)
}

// ─── Session Persistence ─────────────────────────────────────

fn session_path() -> PathBuf {
    PathBuf::from(SESSION_FILE)
}

fn save_session(email: &str, cookies: &[String]) {
    let data = SessionData {
        email: email.to_string(),
        cookies: cookies.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        let _ = fs::write(session_path(), json);
    }
}

fn load_session() -> Option<SessionData> {
    let path = session_path();
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn delete_session() {
    let _ = fs::remove_file(session_path());
}

fn build_client_with_cookies(cookies: &[String]) -> Client {
    let mut headers = header::HeaderMap::new();
    let cookie_str = cookies.join("; ");
    if let Ok(val) = header::HeaderValue::from_str(&cookie_str) {
        headers.insert(header::COOKIE, val);
    }

    Client::builder()
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36")
        .build()
        .expect("Failed to build HTTP client")
}

fn build_client_no_cookies() -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36")
        .build()
        .expect("Failed to build HTTP client")
}

/// Extract Set-Cookie values from a response
fn extract_cookies(res: &reqwest::Response) -> Vec<String> {
    res.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| {
            let full = v.to_str().ok()?;
            // Extract just "name=value" part before first ";"
            let cookie = full.split(';').next().unwrap_or(full);
            Some(cookie.to_string())
        })
        .collect()
}

// ─── Helpers ─────────────────────────────────────────────────

fn format_rpow(base_units: f64) -> String {
    if base_units >= BASE_UNITS_PER_RPOW {
        format!("{:.4} RPOW", base_units / BASE_UNITS_PER_RPOW)
    } else {
        format!("{:.6} RPOW", base_units / BASE_UNITS_PER_RPOW)
    }
}

fn extract_username(email: &str) -> String {
    email.split('@').next().unwrap_or(email).to_string()
}

// ─── Main ────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{}", "╔══════════════════════════════════════════════╗".bright_cyan());
    println!("{}", "║      RPOW2 Live Mining Bot  v2.1             ║".bright_cyan());
    println!("{}", "║      github.com/neoatesya/rpow2_ai_agent     ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════════════════╝".bright_cyan());
    println!();

    // ── Try to restore saved session ──
    let (mut client, mut email) = if let Some(session) = load_session() {
        println!("{} {}", "▸ Found saved session for".bright_yellow(), session.email.bright_blue());
        let test_client = build_client_with_cookies(&session.cookies);

        // Test if session is still valid
        match test_client.get(format!("{}/me", API_BASE)).send().await {
            Ok(res) if res.status().is_success() => {
                println!("{}", "✓ Session restored! Skipping login.".bright_green());
                (test_client, session.email)
            }
            _ => {
                println!("{}", "⚠ Saved session expired. Need to login again.".bright_yellow());
                delete_session();
                do_login().await?
            }
        }
    } else {
        do_login().await?
    };

    let mut username = extract_username(&email);

    // ── Prompt for Worker Count ──
    print!("{} ", "▸ Number of workers (default 2):".bright_yellow());
    io::stdout().flush()?;
    let mut worker_input = String::new();
    io::stdin().read_line(&mut worker_input)?;
    let worker_input = worker_input.trim();
    let cores: usize = if worker_input.is_empty() {
        2
    } else {
        worker_input.parse().unwrap_or(2)
    };

    let max_cores = num_cpus::get();
    let cores = cores.max(1);

    if cores > max_cores {
        println!(
            "{} You have {} CPU cores but set {} workers. Performance may not scale beyond {}.",
            "⚠ Warning:".bright_yellow().bold(),
            max_cores,
            cores,
            max_cores
        );
    }

    // ── Fetch account info ──
    println!("{}", "▸ Fetching account info...".bright_yellow());
    let mut balance_rpow: f64 = 0.0;
    let mut daily_remaining: String = "unknown".to_string();

    if let Ok(me_res) = client.get(format!("{}/me", API_BASE)).send().await {
        if me_res.status().is_success() {
            if let Ok(me) = me_res.json::<MeResponse>().await {
                if let Some(bal) = &me.balance_base_units {
                    if let Ok(b) = bal.parse::<f64>() {
                        balance_rpow = b / BASE_UNITS_PER_RPOW;
                    }
                }
                if let Some(remaining) = &me.daily_remaining_base_units {
                    if let Ok(r) = remaining.parse::<f64>() {
                        daily_remaining = format_rpow(r);
                    }
                }
            }
        }
    }

    println!();
    println!("{}", "─────────────────────────────────────────────".bright_cyan());
    println!("  {}  {}", "User:".dimmed(), username.bright_white().bold());
    println!("  {}  {:.6} RPOW", "Balance:".dimmed(), balance_rpow);
    println!("  {}  {}", "Daily Remaining:".dimmed(), daily_remaining);
    println!("  {}  {}", "Workers:".dimmed(), cores.to_string().bright_white());
    println!("  {}  {}", "Session:".dimmed(), session_path().display().to_string().dimmed());
    println!("{}", "─────────────────────────────────────────────".bright_cyan());
    println!();

    thread::sleep(Duration::from_secs(1));

    let mut session_count: u64 = 0;
    let mut total_rpow_earned: f64 = 0.0;
    let mut auth_fail_count: u32 = 0;

    // ── Infinite Mining Loop ──
    loop {
        let time_str = Local::now().format("%H:%M:%S").to_string().bright_green();
        let user_display = username.bright_cyan();

        println!(
            "{} {}  {}",
            time_str,
            user_display,
            "requesting challenge...".truecolor(255, 165, 0)
        );

        let challenge_res = match client
            .post(format!("{}/challenge", API_BASE))
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .send()
            .await
        {
            Ok(res) => res,
            Err(e) => {
                let time_str = Local::now().format("%H:%M:%S").to_string().bright_green();
                println!(
                    "{} {}  {} {}",
                    time_str, user_display,
                    "✗ NETWORK ERROR".bright_red().bold(), e
                );
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        let status_code = challenge_res.status().as_u16();

        // Handle session expired (401) — retry a few times, then re-login
        // Server overload can cause false 401s
        if status_code == 401 {
            auth_fail_count += 1;
            let body = challenge_res.text().await.unwrap_or_default();
            if auth_fail_count >= 3 {
                println!(
                    "{} {}  {}",
                    time_str, user_display,
                    "✗ Session expired! Re-login required...".bright_yellow().bold()
                );
                delete_session();
                match do_login().await {
                    Ok((new_client, new_email)) => {
                        client = new_client;
                        email = new_email;
                        username = extract_username(&email);
                        auth_fail_count = 0;
                        println!("{}", "✓ Re-login successful! Resuming mining...".bright_green());
                        continue;
                    }
                    Err(e) => {
                        println!("{} {}", "✗ Re-login failed:".bright_red(), e);
                        break;
                    }
                }
            } else {
                println!(
                    "{} {}  {} retry {}/3 in 10s... {}",
                    time_str, user_display,
                    "⚠ AUTH ERROR (401)".bright_yellow().bold(),
                    auth_fail_count,
                    body.dimmed()
                );
                thread::sleep(Duration::from_secs(10));
                continue;
            }
        } else {
            auth_fail_count = 0; // reset on any non-401
        }

        // Handle supply exhausted
        if status_code == 410 {
            println!(
                "{} {}  {}",
                time_str, user_display,
                "✗ SUPPLY EXHAUSTED - 21M cap reached!".bright_red().bold()
            );
            break;
        }

        // Handle cooldown (429)
        if status_code == 429 {
            let cooldown: CooldownResponse = challenge_res.json().await.unwrap_or(CooldownResponse {
                error: None,
                message: None,
                retry_after: Some(5),
            });
            let wait = cooldown.retry_after.unwrap_or(5);
            println!(
                "{} {}  {} waiting {}s...",
                time_str, user_display,
                "⏳ COOLDOWN".bright_yellow(),
                wait
            );
            thread::sleep(Duration::from_secs(wait));
            continue;
        }

        if !challenge_res.status().is_success() {
            let body = challenge_res.text().await.unwrap_or_default();
            println!(
                "{} {}  {} {} {}",
                time_str, user_display,
                "✗ API ERROR".bright_red().bold(),
                status_code,
                body.dimmed()
            );
            thread::sleep(Duration::from_secs(5));
            continue;
        }

        let challenge: ChallengeResponse = match challenge_res.json().await {
            Ok(c) => c,
            Err(e) => {
                println!(
                    "{} {}  {} Failed to parse: {}",
                    time_str, user_display,
                    "✗ ERROR".bright_red().bold(), e
                );
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        let prefix_short = if challenge.nonce_prefix.len() > 16 {
            &challenge.nonce_prefix[..16]
        } else {
            &challenge.nonce_prefix
        };

        let time_str = Local::now().format("%H:%M:%S").to_string().bright_green();
        println!(
            "{} {}  {} diff={}  workers={}  prefix={}...",
            time_str,
            user_display,
            "⛏ mining...".bright_green(),
            challenge.difficulty_bits,
            cores,
            prefix_short
        );

        let (solution, time_secs) =
            mine_challenge(&challenge.nonce_prefix, challenge.difficulty_bits, cores);

        let mint_res = match client
            .post(format!("{}/mint", API_BASE))
            .json(&MintRequest {
                challenge_id: &challenge.challenge_id,
                solution_nonce: solution.to_string(),
            })
            .send()
            .await
        {
            Ok(res) => res,
            Err(e) => {
                let time_str = Local::now().format("%H:%M:%S").to_string().bright_green();
                println!(
                    "{} {}  {} {}",
                    time_str, user_display,
                    "✗ NETWORK ERROR".bright_red().bold(), e
                );
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        let time_str = Local::now().format("%H:%M:%S").to_string().bright_green();
        let mint_status = mint_res.status().as_u16();

        if mint_res.status().is_success() {
            let body: MintResponse = mint_res.json().await.unwrap_or(MintResponse {
                token: None,
                error: None,
                message: None,
            });

            if let Some(ref t) = body.token {
                let token_short = if t.id.len() > 8 { &t.id[..8] } else { &t.id };

                let reward_base = t.value_base_units
                    .as_ref()
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(1_000_000.0);
                let reward_rpow = reward_base / BASE_UNITS_PER_RPOW;

                session_count += 1;
                total_rpow_earned += reward_rpow;

                let rate = (solution as f64 / 1_000_000.0) / time_secs;

                println!(
                    "{} {}  {} +{:.4} RPOW  token={}...  hashes={}  time={:.1}s  rate={:.2} MH/s  session={}  total={:.4}",
                    time_str,
                    user_display,
                    "✓ MINTED".bright_green().bold(),
                    reward_rpow,
                    token_short,
                    solution.to_formatted_string(&Locale::en),
                    time_secs,
                    rate,
                    session_count,
                    total_rpow_earned
                );
            } else {
                println!(
                    "{} {}  {} {}",
                    time_str, user_display,
                    "✗ MINT FAILED".bright_red().bold(),
                    body.message.unwrap_or_else(|| "unknown error".to_string())
                );
            }
        } else {
            let body: MintResponse = mint_res.json().await.unwrap_or(MintResponse {
                token: None,
                error: None,
                message: None,
            });

            let error_type = body.error.as_deref().unwrap_or("UNKNOWN");
            let message = body.message.as_deref().unwrap_or("unknown error");

            match error_type {
                "DAILY_CAP_REACHED" => {
                    println!(
                        "{} {}  {} {}",
                        time_str, user_display,
                        "⚠ DAILY CAP REACHED".bright_yellow().bold(),
                        message
                    );
                    println!(
                        "{} {}  {}",
                        time_str, user_display,
                        "Waiting until UTC midnight reset... sleeping 60s".dimmed()
                    );
                    thread::sleep(Duration::from_secs(60));
                    continue;
                }
                "SUPPLY_EXHAUSTED" => {
                    println!(
                        "{} {}  {} {}",
                        time_str, user_display,
                        "✗ SUPPLY EXHAUSTED".bright_red().bold(),
                        message
                    );
                    break;
                }
                "CHALLENGE_EXPIRED" => {
                    println!(
                        "{} {}  {} {} - retrying...",
                        time_str, user_display,
                        "⚠ CHALLENGE EXPIRED".bright_yellow().bold(),
                        message
                    );
                    continue;
                }
                _ => {
                    println!(
                        "{} {}  {} [{}] {} (status={})",
                        time_str, user_display,
                        "✗ FAILED".bright_red().bold(),
                        error_type,
                        message,
                        mint_status
                    );
                }
            }
        }

        // Server enforces 5s cooldown between challenges
        thread::sleep(Duration::from_secs(5));
    }

    println!("\n{}", "═══════════════════════════════════════════".bright_cyan());
    println!(
        "  Session complete: {} mints, {:.4} RPOW earned",
        session_count, total_rpow_earned
    );
    println!("{}", "═══════════════════════════════════════════".bright_cyan());

    Ok(())
}

// ─── Login Flow ──────────────────────────────────────────────

async fn do_login() -> Result<(Client, String), Box<dyn std::error::Error>> {
    // 1. Prompt for Email
    print!("{} ", "▸ Enter email:".bright_yellow());
    io::stdout().flush()?;
    let mut email_input = String::new();
    io::stdin().read_line(&mut email_input)?;
    let email = email_input.trim().to_string();

    if email.is_empty() {
        println!("{}", "✗ Email cannot be empty!".bright_red());
        std::process::exit(1);
    }

    let temp_client = build_client_no_cookies();

    // 2. Request Magic Link
    println!("{} {}...", "▸ Requesting magic link for".bright_yellow(), email.bright_blue());
    let res = temp_client
        .post(format!("{}/auth/request", API_BASE))
        .json(&AuthRequest {
            email: email.clone(),
        })
        .send()
        .await;

    match res {
        Ok(response) => {
            let status = response.status().as_u16();
            if response.status().is_success() {
                println!("{}", "✓ Magic link sent! Check your email.".bright_green());
            } else if status == 429 {
                println!("{}", "⚠ Rate limited, but check your email for a recent magic link!".bright_yellow());
            } else {
                let text = response.text().await.unwrap_or_default();
                if text.contains("TURNSTILE_REQUIRED") || text.contains("BLOCKED") {
                    println!("{}", "⚠ Server requires CAPTCHA / browser verification.".bright_yellow());
                    println!("{}", "  → Login via https://rpow2.com first, then paste the magic link from your email.".dimmed());
                } else {
                    println!("{} {}", "⚠ Auth request failed:".bright_yellow(), text.dimmed());
                    println!("{}", "  → Try logging in via https://rpow2.com and paste the token from email.".dimmed());
                }
            }
        }
        Err(e) => {
            println!("{} {}", "⚠ Could not reach auth server:".bright_yellow(), e.to_string().dimmed());
            println!("{}", "  → Login via https://rpow2.com and paste the magic link from your email.".dimmed());
        }
    }

    // 3. Paste token
    print!("{} ", "▸ Paste magic link or token:".bright_yellow());
    io::stdout().flush()?;
    let mut magic_input = String::new();
    io::stdin().read_line(&mut magic_input)?;
    let magic_input = magic_input.trim();

    let token = if magic_input.contains("token=") {
        magic_input.split("token=").last().unwrap_or("").to_string()
    } else {
        magic_input.to_string()
    };

    // 4. Verify Token — use no-redirect client to capture Set-Cookie from 302
    println!("{}", "▸ Verifying token...".bright_yellow());

    // Build a special client that does NOT follow redirects
    // so we can capture Set-Cookie from the 302 response
    let no_redirect_client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36")
        .build()?;

    let res = no_redirect_client
        .get(format!("{}/auth/verify", API_BASE))
        .query(&[("token", &token)])
        .send()
        .await?;

    let status = res.status().as_u16();

    // 302 = success (redirect with cookie), 200 = also success
    if status != 302 && status != 200 {
        println!("{} Status: {}", "✗ Failed to verify token.".bright_red(), res.status());
        let text = res.text().await.unwrap_or_default();
        println!("  Response: {}", text);
        std::process::exit(1);
    }

    // Extract cookies from the 302 response
    let cookies = extract_cookies(&res);
    if cookies.is_empty() {
        println!("{}", "⚠ No session cookies received. Session won't be saved.".bright_yellow());
    } else {
        save_session(&email, &cookies);
        println!(
            "{} saved to {}",
            "✓ Session persisted!".bright_green(),
            SESSION_FILE.dimmed()
        );
    }

    println!("{}", "✓ Login successful!".bright_green());

    // Build client with the new cookies
    let client = build_client_with_cookies(&cookies);
    Ok((client, email))
}

