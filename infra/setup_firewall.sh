#!/usr/bin/env bash
set -euo pipefail

# Configure UFW and Docker firewall integration on a remote host.
# Example:
#   HOST=root@116.203.46.179 SSH_OPTS="-p 22" ./infra/setup_firewall.sh
# TAILSCALE_ALLOW=0 will skip tailscale0 allowances.
# ALLOW_HTTP=0 / ALLOW_HTTPS=0 can be used to skip 80/443 if you front with another LB.

HOST="${HOST:-}"
SSH_OPTS="${SSH_OPTS:-}"
TAILSCALE_ALLOW="${TAILSCALE_ALLOW:-1}"
ALLOW_HTTP="${ALLOW_HTTP:-1}"
ALLOW_HTTPS="${ALLOW_HTTPS:-1}"

if [ -z "$HOST" ]; then
  echo "Usage: HOST=user@hostname [SSH_OPTS=\"...\"] [TAILSCALE_ALLOW=0] [ALLOW_HTTP=0] [ALLOW_HTTPS=0] $0" >&2
  exit 1
fi

ssh_nohost() {
  ssh -o "StrictHostKeyChecking no" $SSH_OPTS "$HOST" "$@"
}

echo "Installing ufw on $HOST..."
ssh_nohost "sudo apt-get update && sudo apt-get install -y ufw"

echo "Configuring UFW base policies and ports on $HOST..."
ssh_nohost "sudo ufw default deny incoming && sudo ufw default allow outgoing"
ssh_nohost "sudo ufw allow 22/tcp"
[ "$ALLOW_HTTP" -eq 1 ] && ssh_nohost "sudo ufw allow 80/tcp"
[ "$ALLOW_HTTPS" -eq 1 ] && ssh_nohost "sudo ufw allow 443/tcp"
# Docker / Swarm ports
ssh_nohost "sudo ufw allow 2376/tcp && sudo ufw allow 2377/tcp && sudo ufw allow 7946/tcp && sudo ufw allow 7946/udp && sudo ufw allow 4789/udp"
if [ "$TAILSCALE_ALLOW" -eq 1 ]; then
  ssh_nohost "sudo ufw allow in on tailscale0 proto esp || true"
  ssh_nohost "sudo ufw allow in on tailscale0 || true"
fi
ssh_nohost "sudo ufw --force enable"

echo "Ensuring DOCKER-USER rules are present on $HOST..."
read -r -d '' RULES <<'EOF' || true
# BEGIN UFW AND DOCKER
*filter
:ufw-user-forward - [0:0]
:ufw-docker-logging-deny - [0:0]
:DOCKER-USER - [0:0]
-A DOCKER-USER -j ufw-user-forward

-A DOCKER-USER -j RETURN -s 10.0.0.0/8
-A DOCKER-USER -j RETURN -s 172.16.0.0/12
-A DOCKER-USER -j RETURN -s 192.168.0.0/16
-A DOCKER-USER -j RETURN -i tailscale0

-A DOCKER-USER -p udp -m udp --sport 53 --dport 1024:65535 -j RETURN

-A DOCKER-USER -j ufw-docker-logging-deny -p tcp -m tcp --tcp-flags FIN,SYN,RST,ACK SYN -d 192.168.0.0/16
-A DOCKER-USER -j ufw-docker-logging-deny -p tcp -m tcp --tcp-flags FIN,SYN,RST,ACK SYN -d 10.0.0.0/8
-A DOCKER-USER -j ufw-docker-logging-deny -p tcp -m tcp --tcp-flags FIN,SYN,RST,ACK SYN -d 172.16.0.0/12
-A DOCKER-USER -j ufw-docker-logging-deny -p udp -m udp --dport 0:32767 -d 192.168.0.0/16
-A DOCKER-USER -j ufw-docker-logging-deny -p udp -m udp --dport 0:32767 -d 10.0.0.0/8
-A DOCKER-USER -j ufw-docker-logging-deny -p udp -m udp --dport 0:32767 -d 172.16.0.0/12

-A DOCKER-USER -j RETURN

-A ufw-docker-logging-deny -m limit --limit 3/min --limit-burst 10 -j LOG --log-prefix "[UFW DOCKER BLOCK] "
-A ufw-docker-logging-deny -j DROP

COMMIT
# END UFW AND DOCKER
EOF

ssh_nohost "sudo grep -q '# BEGIN UFW AND DOCKER' /etc/ufw/after.rules || sudo tee -a /etc/ufw/after.rules >/dev/null" <<<"$RULES"

echo "Reloading ufw to apply Docker rules on $HOST..."
ssh_nohost "sudo ufw reload"

echo "Firewall setup complete on $HOST."
