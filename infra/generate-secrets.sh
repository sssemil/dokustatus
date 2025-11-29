#!/usr/bin/env bash
set -euo pipefail

SECRET_DIR="${SECRET_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/secrets}"
LENGTH="${LENGTH:-64}"

mkdir -p "$SECRET_DIR"

gen_secret() {
  local name="$1"
  local file="${SECRET_DIR}/${name}"
  if [ -s "$file" ]; then
    echo "Skipping ${name} (already exists)"
    return
  fi
  head -c "$LENGTH" /dev/urandom | base64 | tr -d '\n' > "$file"
  echo "Wrote ${file}"
}

gen_secret "jwt_secret"
gen_secret "process_number_key"
gen_secret "resend_api_key"
gen_secret "postgres_password"
gen_secret "redis_password"
