#!/bin/bash
# Eseguito una sola volta (via cloud-init) per preparare l'immagine golden.
set -euo pipefail
SRC=/mnt/job
export DEBIAN_FRONTEND=noninteractive

apt-get update -qq
apt-get install -y -qq git curl ca-certificates ripgrep jq build-essential >/dev/null
sudo -u agent -H bash -c 'curl -fsSL https://claude.ai/install.sh | bash'
sudo -u agent -H /home/agent/.local/bin/claude --version
echo 'export PATH="$HOME/.local/bin:$PATH"' >> /home/agent/.bashrc

install -m 755 "$SRC/agentvm-job" /usr/local/bin/agentvm-job
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
