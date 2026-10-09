#!/bin/bash
# Run only once (via cloud-init) to prepare the golden image.
set -euo pipefail
SRC=/mnt/job
export DEBIAN_FRONTEND=noninteractive

apt-get update -qq
apt-get install -y -qq git curl ca-certificates ripgrep jq build-essential tmux python3-pip python3-venv >/dev/null
# The shell of the terminals: zsh with a developer's tools, all from Debian (no install scripts).
apt-get install -y -qq zsh zsh-autosuggestions zsh-syntax-highlighting fzf bat eza zoxide fd-find lazygit starship >/dev/null
install -d /etc/agentvm
install -m 644 "$SRC/config/zshrc" /etc/agentvm/zshrc
install -m 644 "$SRC/config/starship.toml" /etc/agentvm/starship.toml
install -m 644 "$SRC/config/zshrc.stub" /root/.zshrc
# A browser for Claude: headless Chromium driven through the Playwright MCP server.
apt-get install -y -qq --no-install-recommends chromium nodejs npm fonts-liberation fonts-noto-color-emoji >/dev/null
npm install -g --no-fund --no-audit --loglevel=error @playwright/mcp@latest
playwright-mcp --help >/dev/null
# Tailscale, from its signed apt repository: a VM can join the user's tailnet and be reached and
# managed from it. tailscaled starts only for a VM that joins (no cost to every boot).
curl -fsSL https://pkgs.tailscale.com/stable/debian/trixie.noarmor.gpg -o /usr/share/keyrings/tailscale-archive-keyring.gpg
curl -fsSL https://pkgs.tailscale.com/stable/debian/trixie.tailscale-keyring.list -o /etc/apt/sources.list.d/tailscale.list
apt-get update -qq
apt-get install -y -qq tailscale >/dev/null
systemctl disable tailscaled.service
tailscale version | head -1
# Claude Code for root: inside the VM the agent has full permissions (the VM is the sandbox).
VERSION=$(cat "$SRC/claude-version" 2>/dev/null || echo latest)
HOME=/root bash -c "curl -fsSL https://claude.ai/install.sh | bash -s '$VERSION'"
/root/.local/bin/claude --version
echo 'export PATH="$HOME/.local/bin:$PATH"' >> /root/.bashrc

install -m 755 "$SRC/agentvm-boot" /usr/local/bin/agentvm-boot
install -m 755 "$SRC/agentvm-job" /usr/local/bin/agentvm-job
install -m 755 "$SRC/agentvm-pty" /usr/local/bin/agentvm-pty
install -m 755 "$SRC/agentvm-metrics" /usr/local/bin/agentvm-metrics
install -m 755 "$SRC/agentvm-statusline" /usr/local/bin/agentvm-statusline
install -m 755 "$SRC/agentvm-claude" /usr/local/bin/agentvm-claude
install -d /etc/agentvm
install -m 644 "$SRC/config/tmux.conf" /etc/agentvm/tmux.conf
# Claude Code ready to use: no onboarding or trust dialogs; hooks for the status.
install -d -m 700 /root/.claude
install -m 644 "$SRC/config/claude-settings.json" /root/.claude/settings.json
install -m 600 "$SRC/config/claude.json" /root/.claude.json
echo 'alias claude=agentvm-claude' >> /root/.bashrc
modprobe vmw_vsock_virtio_transport 2>/dev/null || true
echo vmw_vsock_virtio_transport > /etc/modules-load.d/agentvm-vsock.conf
install -m 644 "$SRC/agentvm.service" /etc/systemd/system/agentvm.service
systemctl enable agentvm.service

# MAC-independent network (cloud-init would bind it to the golden image's MAC).
rm -f /etc/netplan/*.yaml
install -m 644 "$SRC/10-agentvm.network" /etc/systemd/network/10-agentvm.network
systemctl enable systemd-networkd

# Fast boot: no cloud-init at runtime, no periodic jobs, no GRUB wait.
touch /etc/cloud/cloud-init.disabled
systemctl disable ssh.service ssh.socket 2>/dev/null || true
systemctl mask apt-daily.timer apt-daily-upgrade.timer man-db.timer e2scrub_all.timer fstrim.timer \
  unattended-upgrades.service systemd-networkd-wait-online.service 2>/dev/null || true
sed -i 's/^GRUB_TIMEOUT=.*/GRUB_TIMEOUT=0/' /etc/default/grub
# The kernel's warnings and errors (a panic, out of memory, disk errors) go to the console the
# host keeps in console.log: a VM that freezes says why. Not tty0, which nobody records.
CMDLINE="console=hvc0 loglevel=4"
sed -i "s/^GRUB_CMDLINE_LINUX_DEFAULT=.*/GRUB_CMDLINE_LINUX_DEFAULT=\"$CMDLINE\"/" /etc/default/grub
grep -q '^GRUB_TIMEOUT_STYLE' /etc/default/grub || echo 'GRUB_TIMEOUT_STYLE=hidden' >> /etc/default/grub
update-grub
# /boot/efi is read by the firmware and GRUB before Linux starts, never after: checking its FAT
# and mounting it held every boot back by ~0.7 s.
sed -i -E 's|^([^[:space:]]+[[:space:]]+/boot/efi[[:space:]]+vfat[[:space:]]+)([^[:space:]]+)([[:space:]]+)[0-9]+[[:space:]]+[0-9]+[[:space:]]*$|\1\2,noauto,nofail\3 0 0|' /etc/fstab
grep -q '/boot/efi.*noauto' /etc/fstab
# The only binary format registered is Python's (running .pyc files directly), never used here:
# setting it up stalled every boot by ~0.7 s on the binfmt_misc automount. With no format left
# the service is skipped; a developer who installs one (qemu-user) gets it back.
# (Diverted out of the folder: systemd runs the service while the folder holds any file at all.)
mkdir -p /usr/lib/binfmt.d.disabled
for f in /usr/lib/binfmt.d/python3*.conf; do
  if [ -e "$f" ]; then
    dpkg-divert --quiet --local --rename --divert "/usr/lib/binfmt.d.disabled/${f##*/}" --add "$f"
  fi
done

# Unique identity for each clone (distinct DHCP leases).
# The apt indexes are kept: Claude installs packages right away, without apt-get update.
apt-get clean
truncate -s 0 /etc/machine-id
# Keep the machine id transient (new each boot): restored snapshots must not share DHCP leases.
systemctl mask systemd-machine-id-commit.service
rm -f /var/lib/dbus/machine-id /var/lib/systemd/network/* 2>/dev/null || true

# The kernel GRUB would boot (the newest), its initrd and GRUB's command line, for the host: it
# boots every clone straight into them, without the firmware and GRUB (~0.7 s of every launch).
KVER=$(find /boot -name 'vmlinuz-*' | sed 's|^/boot/vmlinuz-||' | sort -V | tail -1)
install -d "$SRC/boot"
cp "/boot/vmlinuz-$KVER" "$SRC/boot/vmlinuz"
cp "/boot/initrd.img-$KVER" "$SRC/boot/initrd.img"
echo "root=PARTUUID=$(findmnt -no PARTUUID /) ro $CMDLINE" > "$SRC/boot/cmdline"
echo GOLDEN_OK
