use chrono::Local;
use colored::*;
use num_format::{Locale, ToFormattedString};
use reqwest::{cookie::Jar, Client};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    io::{self, Write},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

const API_BASE: &str = "https://api.rpow2.com";
const BASE_UNITS_PER_RPOW: f64 = 1_000_000_000.0; // 9 decimals

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
    daily_mint_cap_base_units: Option<String>,
    daily_minted_base_units: Option<String>,
    daily_remaining_base_units: Option<String>,
}

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
                // Check if another thread found the solution
                if found.load(Ordering::Relaxed) {
                    return None;
                }

                // Inner loop to reduce atomic load overhead
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{}", "╔══════════════════════════════════════════════╗".bright_cyan());
    println!("{}", "║      RPOW2 Live Mining Bot  v2.0             ║".bright_cyan());
    println!("{}", "║      github.com/frkrueger/rpow               ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════════════════╝".bright_cyan());
    println!();

    // 1. Prompt for Email
    print!("{} ", "▸ Enter email:".bright_yellow());
    io::stdout().flush()?;
    let mut email_input = String::new();
    io::stdin().read_line(&mut email_input)?;
    let email = email_input.trim().to_string();

    if email.is_empty() {
        println!("{}", "✗ Email cannot be empty!".bright_red());
        return Ok(());
    }

    let username = extract_username(&email);

    let jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_provider(Arc::clone(&jar))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()?;

    // 2. Request Magic Link
    println!("{} {}...", "▸ Requesting magic link for".bright_yellow(), email.bright_blue());
    let res = client
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
                if text.contains("TURNSTILE_REQUIRED") {
                    println!("{}", "⚠ Server requires CAPTCHA verification.".bright_yellow());
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

    // 3. Verify Token and get Session Cookie
    println!("{}", "▸ Verifying token...".bright_yellow());
    let res = client
        .get(format!("{}/auth/verify", API_BASE))
        .query(&[("token", &token)])
        .send()
        .await?;

    if !res.status().is_success() && res.status().as_u16() != 302 && res.status().as_u16() != 200 {
        println!("{} Status: {}", "✗ Failed to verify token.".bright_red(), res.status());
        let text = res.text().await.unwrap_or_default();
        println!("  Response: {}", text);
        return Ok(());
    }

    println!("{}", "✓ Login successful!".bright_green());

    // 3.5 Prompt for Worker Count
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

    // 4. Fetch initial account info
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
    println!("  {}  0.001 RPOW", "Reward/mint:".dimmed());
    println!("{}", "─────────────────────────────────────────────".bright_cyan());
    println!();

    thread::sleep(Duration::from_secs(1));

    let mut session_count: u64 = 0;
    let mut total_rpow_earned: f64 = 0.0;

    // 5. Infinite Mining Loop
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

        // Handle supply exhausted
        if status_code == 410 {
            println!(
                "{} {}  {}",
                time_str, user_display,
                "✗ SUPPLY EXHAUSTED - 21M cap reached!".bright_red().bold()
            );
            break;
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

                // Parse reward value from response
                let reward_base = t.value_base_units
                    .as_ref()
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(1_000_000.0); // default 0.001 RPOW
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

        thread::sleep(Duration::from_millis(500));
    }

    println!("\n{}", "═══════════════════════════════════════════".bright_cyan());
    println!(
        "  Session complete: {} mints, {:.4} RPOW earned",
        session_count, total_rpow_earned
    );
    println!("{}", "═══════════════════════════════════════════".bright_cyan());

    Ok(())
}
