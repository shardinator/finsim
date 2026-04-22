# FinSim

Minimal Rust + Actix Web + Tera app, ready to deploy on Fly.io.

## Run locally

```bash
cargo run
```

Then open `http://localhost:8080`.

## Deploy to Fly.io

1. Install and authenticate the Fly CLI.
2. From this repo:

```bash
fly launch --no-deploy
```

If Fly suggests overwriting `fly.toml`, keep the existing one.

3. Deploy:

```bash
fly deploy
```

4. Open the app:

```bash
fly open
```
