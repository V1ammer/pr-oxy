#!/bin/bash
set -euo pipefail

REPO="V1ammer/pr-oxy"
PORT="${1:-8080}"
USER="${2:-}"
PASS="${3:-}"

echo "=== pr-oxy install ==="

# Создаём пользователя
if ! id -u pr-oxy &>/dev/null; then
    sudo useradd --system --no-create-home --shell /usr/sbin/nologin pr-oxy
fi

# Директория
sudo mkdir -p /opt/pr-oxy
sudo chown pr-oxy:pr-oxy /opt/pr-oxy
sudo chmod 700 /opt/pr-oxy

# Скачиваем бинарник из последнего релиза GitHub
BINARY_URL="https://github.com/${REPO}/releases/latest/download/pr-oxy"
echo "Downloading ${BINARY_URL} ..."
curl -fsSL -o /tmp/pr-oxy "${BINARY_URL}"

sudo mv /tmp/pr-oxy /opt/pr-oxy/pr-oxy
sudo chmod +x /opt/pr-oxy/pr-oxy
sudo chown pr-oxy:pr-oxy /opt/pr-oxy/pr-oxy

# Создаём .env
sudo tee /opt/pr-oxy/.env > /dev/null <<EOF
PORT=${PORT}
EOF

if [[ -n "$USER" && -n "$PASS" ]]; then
    sudo tee -a /opt/pr-oxy/.env > /dev/null <<EOF
USER=${USER}
PASS=${PASS}
EOF
fi

sudo chown pr-oxy:pr-oxy /opt/pr-oxy/.env
sudo chmod 600 /opt/pr-oxy/.env

# Systemd unit inline
sudo tee /etc/systemd/system/pr-oxy.service > /dev/null <<'EOF'
[Unit]
Description=pr-oxy forward proxy
After=network.target

[Service]
Type=simple
User=pr-oxy
Group=pr-oxy
WorkingDirectory=/opt/pr-oxy
EnvironmentFile=/opt/pr-oxy/.env
ExecStart=/opt/pr-oxy/pr-oxy
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=pr-oxy

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable pr-oxy
sudo systemctl restart pr-oxy

echo "=== done ==="
sudo systemctl status pr-oxy --no-pager
