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

## 🖥️ Step-by-Step Guide

When you run the bot, it will ask 3 things:

### 1️⃣ Enter your email
```
▸ Enter email: your_email@gmail.com
```
Use the same email you registered on [rpow2.com](https://rpow2.com).

### 2️⃣ Paste the magic link
```
▸ Paste magic link or token: <paste from your email>
```
Check your inbox → copy the link or token → paste it.

### 3️⃣ Set number of workers
```
▸ Number of workers (default 2): 2
```
Just press **Enter** for default (2), or type a number (e.g. `4` for faster mining, uses more CPU).

---

## ✅ Output Example

```
11:49:40 your_user  ⛏ mining... diff=24  workers=2  prefix=a1b2c3d4...
11:49:41 your_user  ✓ MINTED +0.0010 RPOW  token=eeebeffa...  time=0.6s  rate=156.96 MH/s  session=1  total=0.0010
```

---

## 💡 Tips

| Tip | Detail |
|-----|--------|
| **Slow mining?** | Make sure you use `cargo run --release` |
| **Rate limited?** | Check your email, the magic link was already sent |
| **Daily cap hit?** | Max 100,000 mints per day per account, resets at UTC midnight |
| **How much per mint?** | 0.001 RPOW per successful mint |

---

## 📜 Credits

Based on [frkrueger/rpow](https://github.com/frkrueger/rpow) — a modern recreation of Hal Finney's Reusable Proofs of Work (2004).
