# ⛏ RPOW2 Mining Bot (Rust)

High-performance, multi-threaded CLI mining bot for [rpow2.com](https://rpow2.com), written in Rust.  
Based on the official [frkrueger/rpow](https://github.com/frkrueger/rpow) project — a modern recreation of Hal Finney's [Reusable Proofs of Work](https://nakamotoinstitute.org/finney/rpow/) (2004).

---

## ✨ Features

- **Multi-threaded SHA-256 mining** — configurable worker count for parallel hashcash solving
- **Optimized hasher** — pre-computes nonce prefix hash state, only appends nonce per iteration
- **Account dashboard** — shows balance, daily quota, and reward info before mining
- **Smart error handling** — auto-retries on network errors, handles daily cap & supply exhaustion
- **Session tracking** — displays running totals of mints and RPOW earned
- **Clean terminal UI** — colored output with timestamps and username display

---

## 📋 Prerequisites

### Install Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Verify installation:

```bash
rustc --version
cargo --version
```

### Register on rpow2.com

1. Go to [rpow2.com](https://rpow2.com)
2. Sign up with your email address
3. You'll use this email to authenticate the mining bot

---

## 🚀 Quick Start

### 1. Clone the repository

```bash
git clone https://github.com/YOUR_USERNAME/rpow2_ai_agent.git
cd rpow2_ai_agent
```

### 2. Build in release mode

> ⚠️ **IMPORTANT**: Always build/run in `--release` mode. Debug mode is ~10x slower.

```bash
cargo build --release
```

### 3. Run the miner

```bash
cargo run --release
```

### 4. Follow the interactive prompts

```
╔══════════════════════════════════════════════╗
║      RPOW2 Live Mining Bot  v2.0             ║
║      github.com/frkrueger/rpow               ║
╚══════════════════════════════════════════════╝

▸ Enter email: your_email@example.com
▸ Requesting magic link for your_email@example.com...
✓ Magic link sent! Check your email.
▸ Paste magic link or token: <paste token from email>
✓ Login successful!
▸ Number of workers (default 2): 4
▸ Fetching account info...

─────────────────────────────────────────────
  User:      your_email
  Balance:   0.107000 RPOW
  Daily Remaining:  99.900000 RPOW
  Workers:   4
  Reward/mint:  0.001 RPOW
─────────────────────────────────────────────

11:49:40 your_email  ⛏ mining... diff=24  workers=4  prefix=a1b2c3d4e5f6...
11:49:41 your_email  ✓ MINTED +0.0010 RPOW  token=eeebeffa...  hashes=101,784,352  time=0.6s  rate=156.96 MH/s  session=1  total=0.0010
```

---

## ⚙️ Configuration

### Worker Count

When prompted `Number of workers (default 2):`, enter the number of CPU threads to use for mining.

| Workers | Best For |
|---------|----------|
| `1-2` | Background mining, laptop |
| `4-8` | Dedicated mining on desktop |
| `8+` | Server / multi-core machine |

- **Default**: 2 workers
- **Maximum**: Automatically capped at your CPU core count
- More workers = faster hash rate, but higher CPU usage

### Magic Link Authentication

The bot uses **magic link** email authentication:

1. Enter your rpow2.com registered email
2. Check your inbox for the magic link email
3. You can paste either:
   - The **full URL** from the email (e.g., `https://rpow2.com/auth/verify?token=abc123...`)
   - Just the **token** value (e.g., `abc123...`)

---

## 📊 RPOW Tokenomics

Information sourced from [frkrueger/rpow](https://github.com/frkrueger/rpow):

| Parameter | Value |
|-----------|-------|
| **Decimals** | 9 (1 RPOW = 1,000,000,000 base units) |
| **Reward per mint** | 0.001 RPOW (1,000,000 base units) |
| **Difficulty** | 24 trailing zero bits (SHA-256) |
| **Max supply** | 19,000,000 RPOW |
| **Halving interval** | Every 1,000,000 RPOW minted |
| **Halving offset** | Starts at 9,000,000 RPOW minted |
| **Daily cap** | 100,000 solutions per account per UTC day |

### Halving Schedule

The reward halves after each 1M RPOW minted (past the 9M offset):

| Phase | Minted Supply | Reward per Mint |
|-------|--------------|-----------------|
| 0 | 0 – 10M RPOW | 0.001 RPOW |
| 1 | 10M – 11M | 0.0005 RPOW |
| 2 | 11M – 12M | 0.00025 RPOW |
| 3 | 12M – 13M | 0.000125 RPOW |
| ... | ... | halves each phase |

---

## 🔧 API Endpoints

The bot interacts with the `api.rpow2.com` server:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/auth/request` | `POST` | Request magic link — body: `{ "email": "..." }` |
| `/auth/verify` | `GET` | Verify token — query: `?token=...` |
| `/challenge` | `POST` | Get mining challenge (nonce_prefix, difficulty) |
| `/mint` | `POST` | Submit solution — body: `{ "challenge_id": "...", "solution_nonce": "..." }` |
| `/me` | `GET` | Get account balance, daily quota, and stats |

---

## 🛠 Proof of Work Algorithm

The mining algorithm uses **SHA-256 with trailing zero bits** (hashcash-style):

1. Server provides a 16-byte `nonce_prefix` (hex) and `difficulty_bits` (e.g., 24)
2. Miner appends an 8-byte **little-endian u64 nonce** to the prefix
3. Computes `SHA-256(nonce_prefix || nonce_le64)`
4. Checks if the hash has `>= difficulty_bits` trailing zero bits
5. Submits the winning nonce to `/mint`

### Optimization

- **Pre-computed base hasher**: The SHA-256 state after hashing the 16-byte prefix is cloned for each nonce attempt, avoiding redundant work
- **Thread striping**: Each thread starts at `thread_id` and increments by `total_threads`, ensuring no duplicate nonce attempts
- **Batch atomic checks**: Atomic flag checked every 4096 iterations to reduce synchronization overhead

---

## 📁 Project Structure

```
rpow2_ai_agent/
├── .gitignore       # Excludes target/ and IDE files
├── Cargo.toml       # Rust dependencies
├── Cargo.lock       # Locked dependency versions
├── README.md        # This file
└── src/
    └── main.rs      # All bot logic (auth, mining, display)
```

---

## ⚠️ Troubleshooting

### SSL/TLS errors on WSL/Ubuntu

The bot uses `rustls-tls` instead of `native-tls` to avoid OpenSSL issues on WSL. This is already configured in `Cargo.toml`. If you see SSL errors, ensure you're using the project's Cargo.toml as-is.

### Rate limited (429)

If you get rate-limited on `/auth/request`, the server already sent a magic link recently. Check your email inbox (including spam folder).

### Daily cap reached

The server enforces a per-account daily limit of 100,000 mints per UTC day. The bot will automatically sleep and retry every 60 seconds until the UTC midnight reset.

### Challenge expired

Challenges expire after 5 minutes. If mining takes too long (unlikely with release mode), the bot auto-retries with a fresh challenge.

### Low hash rate

Make sure you're running in **release mode**:

```bash
cargo run --release    # ✅ Fast (~100+ MH/s)
cargo run              # ❌ Slow (~5 MH/s) — debug mode
```

---

## 📜 License

This project is for educational purposes. Based on the open-source [rpow](https://github.com/frkrueger/rpow) project by frkrueger.
