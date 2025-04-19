#!/bin/bash
set -euo pipefail
IFS=$'\n\t'

# Flush existing rules and delete existing ipsets
iptables -F
iptables -X
iptables -t nat -F
iptables -t nat -X
iptables -t mangle -F
iptables -t mangle -X
ipset destroy allowed-domains 2>/dev/null || true

# Allow DNS & localhost
iptables -A OUTPUT -p udp --dport 53 -j ACCEPT
iptables -A INPUT -p udp --sport 53 -j ACCEPT
iptables -A INPUT -i lo -j ACCEPT
iptables -A OUTPUT -o lo -j ACCEPT

# Create ipset
ipset create allowed-domains hash:net

# Add domains to allowlist (common domains)
ALLOWED_DOMAINS=(
    # 🔑 OpenAI
    "api.openai.com"

    # 🟨 Node.js (npm)
    "registry.npmjs.org"

    # 🐍 Python (pip)
    "pypi.org"
    "files.pythonhosted.org"

    # 🟦 Go (modules)
    "proxy.golang.org"
    "sum.golang.org"
    "storage.googleapis.com"

    # 🦀 Rust (cargo)
    "crates.io"
    "static.crates.io"
    "github.com"
    "objects.githubusercontent.com"

    # ☕ Java (Maven, Gradle)
    "repo1.maven.org"
    "repo.maven.apache.org"
    "jcenter.bintray.com"
    "plugins.gradle.org"
    "services.gradle.org"

    # 🎯 C# / .NET (NuGet)
    "api.nuget.org"
    "www.nuget.org"
    "globalcdn.nuget.org"

    # 🐧 Linux distros (package mirrors)
    "downloads.sourceforge.net"
    "dl-cdn.alpinelinux.org"
    "deb.debian.org"
    "security.debian.org"

    # 🐳 Docker
    "registry-1.docker.io"
    "auth.docker.io"
    "production.cloudflare.docker.com"
)

for domain in "${ALLOWED_DOMAINS[@]}"; do
    echo "Resolving $domain..."
    ips=$(dig +short A "$domain")
    if [ -z "$ips" ]; then
        echo "ERROR: Failed to resolve $domain"
        exit 1
    fi

    while read -r ip; do
        if [[ "$ip" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            echo "Adding $ip for $domain"
            ipset add allowed-domains "$ip"
        else
            echo "WARNING: Skipped invalid IP for $domain: $ip"
        fi
    done < <(echo "$ips")
done

# Detect host IP
HOST_IP=$(ip route | grep default | cut -d" " -f3)
[ -z "$HOST_IP" ] && { echo "ERROR: Failed to detect host IP"; exit 1; }

HOST_NETWORK=$(echo "$HOST_IP" | sed "s/\.[0-9]*$/.0\/24/")
echo "Host network detected as: $HOST_NETWORK"

iptables -A INPUT -s "$HOST_NETWORK" -j ACCEPT
iptables -A OUTPUT -d "$HOST_NETWORK" -j ACCEPT

# Set default DROP policy
iptables -P INPUT DROP
iptables -P FORWARD DROP
iptables -P OUTPUT DROP

# Allow existing connections
iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT

# Allow outbound traffic to IPs in allowed-domains
iptables -A OUTPUT -m set --match-set allowed-domains dst -j ACCEPT

# Reject everything else (clean failure response)
iptables -A INPUT -p tcp -j REJECT --reject-with tcp-reset
iptables -A INPUT -p udp -j REJECT --reject-with icmp-port-unreachable
iptables -A OUTPUT -p tcp -j REJECT --reject-with tcp-reset
iptables -A OUTPUT -p udp -j REJECT --reject-with icmp-port-unreachable
iptables -A FORWARD -p tcp -j REJECT --reject-with tcp-reset
iptables -A FORWARD -p udp -j REJECT --reject-with icmp-port-unreachable

echo "Firewall configuration complete"

# Basic verification
echo "Verifying blocked domain..."
if curl --connect-timeout 5 https://example.com >/dev/null 2>&1; then
    echo "ERROR: Unexpected access to https://example.com"
    exit 1
else
    echo "✔️ Firewall correctly blocks example.com"
fi

echo "Verifying access to package registries..."
for test_domain in \
    "https://api.openai.com" \
    "https://registry.npmjs.org" \
    "https://pypi.org/simple" \
    "https://github.com"; do
    if curl --connect-timeout 5 --head "$test_domain" >/dev/null 2>&1; then
        echo "✔️ Able to reach $test_domain"
    else
        echo "❌ Failed to reach $test_domain"
        exit 1
    fi
done
