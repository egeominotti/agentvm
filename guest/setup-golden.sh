#!/bin/bash
# Eseguito una sola volta (via cloud-init) per preparare l'immagine golden.
set -euo pipefail
SRC=/mnt/job
export DEBIAN_FRONTEND=noninteractive

apt-get update -qq
apt-get install -y -qq git curl ca-certificates ripgrep jq build-essential tmux python3-pip python3-venv >/dev/null
sudo -u agent -H bash -c 'curl -fsSL https://claude.ai/install.sh | bash'
sudo -u agent -H /home/agent/.local/bin/claude --version
echo 'export PATH="$HOME/.local/bin:$PATH"' >> /home/agent/.bashrc

install -m 755 "$SRC/agentvm-job" /usr/local/bin/agentvm-job
install -m 755 "$SRC/agentvm-pty" /usr/local/bin/agentvm-pty
install -m 755 "$SRC/agentvm-claude" /usr/local/bin/agentvm-claude
install -d /etc/agentvm
install -m 644 "$SRC/config/tmux.conf" /etc/agentvm/tmux.conf
# Claude Code pronto all'uso: niente onboarding né dialoghi di fiducia; hook per lo stato.
sudo -u agent -H mkdir -p /home/agent/.claude
install -o agent -g agent -m 644 "$SRC/config/claude-settings.json" /home/agent/.claude/settings.json
install -o agent -g agent -m 600 "$SRC/config/claude.json" /home/agent/.claude.json
echo 'alias claude=agentvm-claude' >> /home/agent/.bashrc
modprobe vmw_vsock_virtio_transport 2>/dev/null || true
echo vmw_vsock_virtio_transport > /etc/modules-load.d/agentvm-vsock.conf
install -m 644 "$SRC/agentvm.service" /etc/systemd/system/agentvm.service
systemctl enable agentvm.service

# Rete indipendente dal MAC (cloud-init la legherebbe a quello della golden).
rm -f /etc/netplan/*.yaml
install -m 644 "$SRC/10-agentvm.network" /etc/systemd/network/10-agentvm.network
systemctl enable systemd-networkd

# Avvio rapido: niente cloud-init a runtime, niente job periodici, niente attesa di GRUB.
touch /etc/cloud/cloud-init.disabled
systemctl disable ssh.service ssh.socket 2>/dev/null || true
systemctl mask apt-daily.timer apt-daily-upgrade.timer man-db.timer e2scrub_all.timer fstrim.timer \
  unattended-upgrades.service systemd-networkd-wait-online.service 2>/dev/null || true
sed -i 's/^GRUB_TIMEOUT=.*/GRUB_TIMEOUT=0/' /etc/default/grub
grep -q '^GRUB_TIMEOUT_STYLE' /etc/default/grub || echo 'GRUB_TIMEOUT_STYLE=hidden' >> /etc/default/grub
update-grub

# Identità unica per ogni clone (lease DHCP distinti).
apt-get clean
rm -rf /var/lib/apt/lists/*
truncate -s 0 /etc/machine-id
rm -f /var/lib/dbus/machine-id /var/lib/systemd/network/* 2>/dev/null || true
echo GOLDEN_OK
