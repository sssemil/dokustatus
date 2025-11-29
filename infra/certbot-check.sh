#!/usr/bin/env sh
set -euo

if [ -z "${CERTBOT_DOMAINS:-}" ]; then
  echo "CERTBOT_DOMAINS is required (comma-separated list of domains)" >&2
  exit 1
fi

if [ -z "${CERTBOT_EMAIL:-}" ]; then
  echo "CERTBOT_EMAIL is required" >&2
  exit 1
fi

DOMAIN_LIST=$(printf "%s" "$CERTBOT_DOMAINS" | tr ',' ' ')
PRIMARY_DOMAIN=$(printf "%s" "$DOMAIN_LIST" | awk '{print $1}')

STAGING_FLAG=
if [ "${CERTBOT_STAGING:-false}" = "true" ]; then
  STAGING_FLAG="--staging"
fi

if [ ! -f "/etc/letsencrypt/live/${PRIMARY_DOMAIN}/fullchain.pem" ]; then
  certbot certonly --non-interactive --agree-tos $STAGING_FLAG \
    --webroot -w /var/www/certbot \
    --email "$CERTBOT_EMAIL" \
    $(printf -- " -d %s" $DOMAIN_LIST)
else
  certbot renew --non-interactive $STAGING_FLAG --webroot -w /var/www/certbot
fi
