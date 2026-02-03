#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

allowed_domains_file="/etc/codex/allowed_domains.txt"
include_github_meta_ranges="${CODEX_INCLUDE_GITHUB_META_RANGES:-1}"
include_cloudflare_ranges="${CODEX_INCLUDE_CLOUDFLARE_RANGES:-1}"

if [ -f "$allowed_domains_file" ]; then
  mapfile -t allowed_domains < <(sed '/^\s*#/d;/^\s*$/d' "$allowed_domains_file")
else
  allowed_domains=("api.openai.com")
fi

if [ "${#allowed_domains[@]}" -eq 0 ]; then
  echo "ERROR: No allowed domains configured"
  exit 1
fi

add_ipv4_cidr_to_allowlist() {
  local source="$1"
  local cidr="$2"

  if [[ ! "$cidr" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]]; then
    echo "ERROR: Invalid ${source} CIDR range: $cidr"
    exit 1
  fi

  ipset add allowed-domains "$cidr" -exist
}

configure_ipv6_default_deny() {
  if ! command -v ip6tables >/dev/null 2>&1; then
    echo "ERROR: ip6tables is required to enforce IPv6 default-deny policy"
    exit 1
  fi

  ip6tables -F
  ip6tables -X
  ip6tables -t mangle -F
  ip6tables -t mangle -X
  ip6tables -t nat -F 2>/dev/null || true
  ip6tables -t nat -X 2>/dev/null || true

  ip6tables -A INPUT -i lo -j ACCEPT
  ip6tables -A OUTPUT -o lo -j ACCEPT
  ip6tables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
  ip6tables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT

  ip6tables -P INPUT DROP
  ip6tables -P FORWARD DROP
  ip6tables -P OUTPUT DROP

  echo "IPv6 firewall policy configured (default-deny)"
}

# Preserve docker-managed DNS NAT rules before clearing tables.
docker_dns_rules="$(iptables-save -t nat | grep "127\\.0\\.0\\.11" || true)"

iptables -F
iptables -X
iptables -t nat -F
iptables -t nat -X
iptables -t mangle -F
iptables -t mangle -X
ipset destroy allowed-domains 2>/dev/null || true

if [ -n "$docker_dns_rules" ]; then
  echo "Restoring Docker DNS NAT rules"
  iptables -t nat -N DOCKER_OUTPUT 2>/dev/null || true
  iptables -t nat -N DOCKER_POSTROUTING 2>/dev/null || true
  while IFS= read -r rule; do
    [ -z "$rule" ] && continue
    iptables -t nat $rule
  done <<< "$docker_dns_rules"
fi

# Allow DNS resolution and localhost communication.
iptables -A OUTPUT -p udp --dport 53 -j ACCEPT
iptables -A OUTPUT -p tcp --dport 53 -j ACCEPT
iptables -A INPUT -p udp --sport 53 -j ACCEPT
iptables -A INPUT -p tcp --sport 53 -j ACCEPT
iptables -A INPUT -i lo -j ACCEPT
iptables -A OUTPUT -o lo -j ACCEPT

ipset create allowed-domains hash:net

for domain in "${allowed_domains[@]}"; do
  echo "Resolving $domain"
  ips="$(dig +short A "$domain" | sed '/^\s*$/d')"
  if [ -z "$ips" ]; then
    echo "ERROR: Failed to resolve $domain"
    exit 1
  fi

  while IFS= read -r ip; do
    if [[ ! "$ip" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]]; then
      echo "ERROR: Invalid IPv4 address from DNS for $domain: $ip"
      exit 1
    fi
    ipset add allowed-domains "$ip" -exist
  done <<< "$ips"
done

if [ "$include_github_meta_ranges" = "1" ]; then
  echo "Fetching GitHub meta ranges"
  github_meta="$(curl -fsSL --connect-timeout 10 https://api.github.com/meta)"

  if ! echo "$github_meta" | jq -e '.web and .api and .git' >/dev/null; then
    echo "ERROR: GitHub meta response missing expected fields"
    exit 1
  fi

  while IFS= read -r cidr; do
    [ -z "$cidr" ] && continue
    if [[ "$cidr" == *:* ]]; then
      # Current policy enforces IPv4-only ipset entries.
      continue
    fi
    add_ipv4_cidr_to_allowlist "GitHub" "$cidr"
  done < <(echo "$github_meta" | jq -r '((.web // []) + (.api // []) + (.git // []))[]' | sort -u)
fi

if [ "$include_cloudflare_ranges" = "1" ]; then
  echo "Fetching Cloudflare IPv4 ranges"
  cloudflare_ranges="$(curl -fsSL --connect-timeout 10 https://www.cloudflare.com/ips-v4)"

  if [ -z "$(echo "$cloudflare_ranges" | sed '/^\s*#/d;/^\s*$/d')" ]; then
    echo "ERROR: Cloudflare range response was empty"
    exit 1
  fi

  while IFS= read -r cidr; do
    [ -z "$cidr" ] && continue
    add_ipv4_cidr_to_allowlist "Cloudflare" "$cidr"
  done < <(echo "$cloudflare_ranges" | sed '/^\s*#/d;/^\s*$/d' | sort -u)
fi

host_ip="$(ip route | awk '/default/ {print $3; exit}')"
if [ -z "$host_ip" ]; then
  echo "ERROR: Failed to detect host IP"
  exit 1
fi

host_network="$(echo "$host_ip" | sed 's/\.[0-9]*$/.0\/24/')"
iptables -A INPUT -s "$host_network" -j ACCEPT
iptables -A OUTPUT -d "$host_network" -j ACCEPT

iptables -P INPUT DROP
iptables -P FORWARD DROP
iptables -P OUTPUT DROP

iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -m set --match-set allowed-domains dst -j ACCEPT

# Reject rather than silently drop to make policy failures obvious.
iptables -A INPUT -j REJECT --reject-with icmp-admin-prohibited
iptables -A OUTPUT -j REJECT --reject-with icmp-admin-prohibited
iptables -A FORWARD -j REJECT --reject-with icmp-admin-prohibited

configure_ipv6_default_deny

echo "Firewall configuration complete"

if curl --connect-timeout 5 https://example.com >/dev/null 2>&1; then
  echo "ERROR: Firewall verification failed - was able to reach https://example.com"
  exit 1
fi

if ! curl --connect-timeout 5 https://api.openai.com >/dev/null 2>&1; then
  echo "ERROR: Firewall verification failed - unable to reach https://api.openai.com"
  exit 1
fi

if [ "$include_github_meta_ranges" = "1" ] && ! curl --connect-timeout 5 https://api.github.com/zen >/dev/null 2>&1; then
  echo "ERROR: Firewall verification failed - unable to reach https://api.github.com"
  exit 1
fi

if curl --connect-timeout 5 -6 https://example.com >/dev/null 2>&1; then
  echo "ERROR: Firewall verification failed - was able to reach https://example.com over IPv6"
  exit 1
fi

echo "Firewall verification passed"
