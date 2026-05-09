# ⛏ RPOW2 Mining Bot

Automated mining bot for [rpow2.com](https://rpow2.com) written in Rust.  
Each successful mint earns **0.001 RPOW**.

---

## 📦 Install Rust (skip if already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env
```

---

## 🚀 How to Run

```bash
git clone https://github.com/neoatesya/rpow2_ai_agent.git
cd rpow2_ai_agent
cargo run --release
```

> ⚠️ Always use `--release` for maximum speed.

---

## 📖 Step-by-Step Guide

### 1. Get your magic link first

Before running the bot, get a login token from the website:

1. Open [rpow2.com](https://rpow2.com) in your browser
2. Click **Mine** → enter your email → complete the CAPTCHA
3. Check your **email inbox** for the magic link
4. **Copy the link** — looks like: `https://rpow2.com/auth/verify?token=abc123...`

### 2. Run the bot

```bash
cargo run --release
```

### 3. Enter your email

```
▸ Enter email: your_email@gmail.com
```

> You'll see a CAPTCHA warning — **that's normal, just ignore it.**

### 4. Paste the magic link from step 1

```
⚠ Server requires CAPTCHA verification.
  → Login via https://rpow2.com first, then paste the magic link from your email.
▸ Paste magic link or token: https://rpow2.com/auth/verify?token=abc123...
```

### 5. Set number of workers

```
▸ Number of workers (default 2): 4
```

Press **Enter** for default (2) or type a number.

### 6. Done! Mining starts automatically 🚀

```
11:49:40 your_user  ⛏ mining... diff=24  workers=4  prefix=a1b2c3d4...
11:49:41 your_user  ✓ MINTED +0.0010 RPOW  token=eeebeffa...  time=0.6s  session=1  total=0.0010
```

---

## 💡 Tips

| Problem | Solution |
|---------|----------|
| Slow mining? | Use `cargo run --release` (not `cargo run`) |
| `TURNSTILE_REQUIRED`? | That's normal! Get token from [rpow2.com](https://rpow2.com) website first |
| `DAILY_CAP_REACHED`? | Max 100k mints/day, resets at UTC midnight |
| `504 error`? | Server is down, bot will auto-retry |
| Token expired? | Get a new one from rpow2.com |

---

## 📜 Credits

Based on [frkrueger/rpow](https://github.com/frkrueger/rpow) — a modern recreation of Hal Finney's Reusable Proofs of Work (2004).
