# Fedora Login Fix Checklist

## Problem Summary
After installing Ubuntu alongside Fedora, the Fedora login stopped working:
- Password accepted at SDDM login screen
- Session starts but immediately crashes
- Returns to login screen (login loop)

## Root Cause
1. **Permissions changed on entire Fedora filesystem** after Ubuntu installation
2. **KWin Wayland compositor crashing** due to graphics buffer issues
3. Error: "QSGContext::initialize: depth buffer support missing, expect rendering errors"
4. Error: "QSGContext::initialize: stencil buffer support missing, expect rendering errors"

## Fixes Applied

### 1. Restore System File Permissions & Ownership ✅

```bash
# Mount Fedora partition
sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora_fix
sudo mount -o subvol=root /dev/mapper/fedora_fix /mnt/fedora
sudo mount -o subvol=home /dev/mapper/fedora_fix /mnt/fedora/home

# Mount pseudo-filesystems for chroot
for dir in proc sys dev dev/pts run; do 
    sudo mount --bind /$dir /mnt/fedora/$dir
done

# Restore all RPM package permissions and ownership
sudo chroot /mnt/fedora /bin/bash -c "rpm --setperms -a"
sudo chroot /mnt/fedora /bin/bash -c "rpm --setugids -a"
```

### 2. Regenerate Initramfs ⏳ (In Progress)

```bash
# Regenerate all initramfs images with proper drivers
sudo chroot /mnt/fedora /bin/bash -c "dracut -f --regenerate-all"
```

### 3. Additional Fixes to Try

#### Option A: Reset KDE/Plasma Configuration (Safe)

```bash
# Backup and reset user's KDE config
sudo chroot /mnt/fedora /bin/bash -c "su - torstein.sornes -c 'cd ~ && tar -czf kde-config-backup.tar.gz .config/plasma* .config/kwin* .local/share/kwin'"
sudo chroot /mnt/fedora /bin/bash -c "su - torstein.sornes -c 'rm -rf ~/.config/plasma* ~/.config/kwin* ~/.local/share/kwin'"
```

#### Option B: Force Software Rendering (Temporary Workaround)

Create `/mnt/fedora/etc/profile.d/force-software-rendering.sh`:
```bash
#!/bin/bash
export LIBGL_ALWAYS_SOFTWARE=1
export QT_XCB_GL_INTEGRATION=none
```

Make it executable:
```bash
sudo chmod +x /mnt/fedora/etc/profile.d/force-software-rendering.sh
```

#### Option C: Update Mesa/Graphics Drivers

```bash
# Update graphics stack
sudo chroot /mnt/fedora /bin/bash -c "dnf update -y mesa-* xorg-x11-drv-* kernel"
```

#### Option D: Recreate SDDM State Directory

```bash
# Clean SDDM state
sudo rm -rf /mnt/fedora/var/lib/sddm/.cache
sudo rm -rf /mnt/fedora/var/lib/sddm/.local
sudo chroot /mnt/fedora /bin/bash -c "systemctl restart sddm"
```

#### Option E: Check and Fix Graphics Device Permissions

```bash
# Ensure proper permissions on graphics devices
sudo chroot /mnt/fedora /bin/bash -c "ls -l /dev/dri/*"
# Should show: crw-rw----+ 1 root video for renderD* and card*

# Fix if needed
sudo chroot /mnt/fedora /bin/bash -c "chmod 0660 /dev/dri/*"
sudo chroot /mnt/fedora /bin/bash -c "chown root:video /dev/dri/*"
```

#### Option F: Verify User in Correct Groups

```bash
# Check user groups
sudo chroot /mnt/fedora /bin/bash -c "groups torstein.sornes"
# Should include: wheel, video, render

# Add to video/render groups if missing
sudo chroot /mnt/fedora /bin/bash -c "usermod -aG video,render torstein.sornes"
```

## Testing Procedure

### From Ubuntu (Current System)

1. Complete all fixes above
2. Unmount Fedora cleanly:
   ```bash
   for dir in proc sys dev/pts dev run; do 
       sudo umount /mnt/fedora/$dir 2>/dev/null
   done
   sudo umount /mnt/fedora/home
   sudo umount /mnt/fedora
   sudo cryptsetup luksClose fedora_fix
   ```

3. Reboot into Fedora:
   ```bash
   sudo reboot
   ```

### From Fedora (After Reboot)

4. Try logging in with Wayland session
5. If login loop persists, try from TTY:
   - Press `Ctrl+Alt+F3` at login screen
   - Login as user
   - Check logs:
     ```bash
     journalctl -b -p err
     journalctl -b | grep -i "kwin\|plasma\|sddm"
     ```

6. If Wayland still fails, try X11:
   - At SDDM login screen, click session type (bottom left)
   - Select "Plasma (X11)" instead of "Plasma (Wayland)"
   - Login

## Verification Commands

```bash
# Check graphics hardware
lspci | grep -i vga
# Should show: Intel Corporation Raptor Lake-P [Iris Xe Graphics]

# Check loaded DRM module
lsmod | grep i915
# Should show: i915 module loaded

# Check OpenGL info
glxinfo | grep -i "opengl renderer"

# Check Wayland compositor
echo $XDG_SESSION_TYPE
# Should be "wayland" or "x11"

# Check for errors
dmesg | grep -i "drm\|i915" | tail -20
```

## Expected Results

After fixes:
- ✅ Login screen appears normally
- ✅ Password accepted
- ✅ Desktop environment loads
- ✅ No return to login screen

## Rollback Plan

If login still fails after all fixes:

1. Boot from Ubuntu Live USB
2. Mount Fedora and restore backup:
   ```bash
   sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora_restore
   sudo mount -o subvol=root /dev/mapper/fedora_restore /mnt/fedora
   # Restore any configs you backed up
   ```

3. Or reinstall Fedora desktop:
   ```bash
   sudo chroot /mnt/fedora /bin/bash
   dnf groupinstall -y "KDE Plasma Workspaces"
   ```

## Notes

- Graphics: Intel Iris Xe (Raptor Lake-P)
- Driver: i915 (built into kernel)
- Mesa version: 25.1.9
- Kernel: 6.17.x
- Display Server: SDDM (Wayland by default)
- Desktop: KDE Plasma

## Related Issues

- Permission changes broke system file ownership
- LUKS encryption requires unlocking before fixes
- Btrfs subvolumes need proper mounting (root and home)
- initramfs must include proper graphics drivers

## Status

- [x] Identified root cause (permission changes + graphics crash)
- [x] Restored RPM package permissions
- [ ] Regenerated initramfs (in progress)
- [ ] Tested login (pending reboot)
- [ ] Verified Wayland works (pending)

---

**Last Updated**: 2025-12-25
**System**: Dell Latitude 7340 with Intel Iris Xe Graphics