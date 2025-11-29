# Dokustatus Infra

Scripts and compose files for building images, shipping them to a remote host over SSH, and running the stack with Nginx + Certbot.

## Files
- `compose.yml` — deploys postgres, redis, api, ui, nginx-http/https, certbot.
- `.env.example` — copy to `.env` and fill values (domain, email, API URLs, DB/Redis, rate limits).
- `secrets/` — place sensitive values as files (see below). Directory is gitignored.
- `deploy.sh` — builds images, syncs infra assets to the target server, loads images, and runs `docker compose up -d`.
- `setup_firewall.sh` — configures UFW + Docker firewall rules on a remote host (installs ufw/iptables if missing).

## Secrets directory
Create text files containing only the secret value:
- `infra/secrets/jwt_secret` — JWT signing key.
- `infra/secrets/process_number_key` — base64 key for encrypting process numbers (PROCESS_NUMBER_KEY).
- `infra/secrets/resend_api_key` — Resend API key.
- `infra/secrets/postgres_password` — Postgres password (used by the DB container and to build `DATABASE_URL` for the API).
- `infra/secrets/redis_password` — Redis password (used by Redis and to build `REDIS_URL` for the API).

These get mounted as Docker secrets and exported to env vars for the API container.

To generate fresh random secrets (skips files that already exist):
```bash
./infra/generate-secrets.sh
```

If Docker builds need host DNS/proxy, you can pass build args to both images:
```bash
BUILD_ARGS="--network=host" ./infra/deploy.sh
```

To harden a host firewall with UFW + Docker integration:
```bash
HOST=root@your-host ./infra/setup_firewall.sh
# optional toggles: SSH_OPTS="-p 2222" TAILSCALE_ALLOW=0 ALLOW_HTTP=0 ALLOW_HTTPS=0
```

## Environment (.env)
Copy `.env.example` to `.env` and adjust:
- Domain: set `LETSENCRYPT_PRIMARY_DOMAIN` and `LETSENCRYPT_DOMAINS` (e.g. `dokustatus.de,dokustatus.de`).
- Cert email: `LETSENCRYPT_EMAIL`.
- API/UI: `NEXT_PUBLIC_API_BASE_URL`, `APP_ORIGIN`, `API_PORT`, `UI_PORT`.
- Backend config: DB/Redis hosts/ports/users, `EMAIL_FROM`, optional `PASS_STATUS_*`, rate limits.

## Deploy
Example to deploy to `root@116.203.46.179` into `/opt/dokustatus`:
```bash
DEPLOY_HOST=116.203.46.179 \
DEPLOY_USER=root \
REMOTE_DIR=/opt/dokustatus \
./infra/deploy.sh
```
You can pass SSH options (e.g. non-standard port) via `SSH_OPTS="-p 2222"`.

The script will:
1. Build local images (reuses `build-images.sh`).
2. Save them as tarballs and rsync them, along with `compose.yml`, `.env`, nginx templates, `certbot-check.sh`, and `secrets/`, to the remote host.
3. Load the images and run `docker compose -f compose.yml --env-file .env up -d` on the server.

Ensure Docker is installed on the target host and that you can SSH as the specified user.
