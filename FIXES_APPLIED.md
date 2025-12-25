# Fedora Login Fix - Summary of Fixes Applied

**Date**: December 25, 2025
**System**: Dell Latitude 7340 with Intel Iris Xe Graphics
**Issue**: Fedora login loop after Ubuntu installation (password accepted but returns to login screen)

---

## Root Cause Analysis

1. **Permission corruption**: Entire Fedora filesystem had permissions/ownership changed after Ubuntu installation
2. **Graphics driver issue**: KWin Wayland compositor crashing with errors:
   - "QSGContext::initialize: depth buffer support missing"
   - "QSGContext::initialize: stencil buffer support missing"
3. **Missing group memberships**: User not in `video` and `render` groups
4. **Corrupted KDE configs**: Plasma/KWin configuration files potentially corrupted

---

## Fixes Applied

### 1. ✅ Restored System File Permissions and Ownership

```bash
# Mounted Fedora partition
sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora_fix
sudo mount -o subvol=root /dev/mapper/luks-5410cf79-bdca-4c37-b640-91c389f40461 /mnt/fedora
sudo mount -o subvol=home /dev/mapper/luks-5410cf79-bdca-4c37-b640-91c389f40461 /mnt/fedora/home

# Mounted pseudo-filesystems for chroot
for dir in proc sys dev dev/pts run; do 
    sudo mount --bind /$dir /mnt/fedora/$dir
done

# Restored all RPM package permissions
sudo chroot /mnt/fedora /bin/bash -c "rpm --setperms -a"

# Restored all RPM package ownership
sudo chroot /mnt/fedora /bin/bash -c "rpm --setugids -a"
```

**Result**: All system files from RPM packages now have correct permissions and ownership.

---

### 2. ✅ Regenerated Initramfs

```bash
# Regenerated all kernel initramfs images with proper drivers
sudo chroot /mnt/fedora /bin/bash -c "dracut -f --regenerate-all"
```

**Result**: All initramfs images (kernels 6.17.11, 6.17.12, 6.17.13) regenerated with correct graphics drivers.

---

### 3. ✅ Added User to Graphics Groups

```bash
# Added user to video and render groups for graphics access
sudo chroot /mnt/fedora /bin/bash -c "usermod -aG video,render torstein.sornes"

# Verified groups
sudo chroot /mnt/fedora /bin/bash -c "groups torstein.sornes"
# Output: torstein.sornes : docker wheel video render
```

**Result**: User now has proper permissions to access graphics hardware.

---

### 4. ✅ Reset KDE/Plasma Configuration

```bash
# Backed up existing KDE configuration
sudo chroot /mnt/fedora /bin/bash -c "su - torstein.sornes -c 'cd ~ && tar -czf kde-config-backup-20251225.tar.gz .config/plasma* .config/kwin* .local/share/kwin 2>/dev/null'"

# Removed potentially corrupted KDE configs
sudo chroot /mnt/fedora /bin/bash -c "cd /home/torstein.sornes/.config && rm -r plasma* kwin* 2>/dev/null"
sudo chroot /mnt/fedora /bin/bash -c "cd /home/torstein.sornes/.local/share && rm -r kwin 2>/dev/null"
```

**Result**: KDE Plasma will regenerate configuration files with correct defaults on next login.

---

## Expected Outcome

After rebooting into Fedora, the following should work:

1. ✅ Login screen appears normally (SDDM)
2. ✅ Password is accepted
3. ✅ Plasma/Wayland session starts successfully
4. ✅ No return to login screen
5. ✅ Full desktop environment loads

---

## Testing Instructions

### Step 1: Reboot into Fedora

```bash
sudo reboot
```

At GRUB menu, select Fedora.

### Step 2: Login

1. Enter password at SDDM login screen
2. Desktop should load successfully

### Step 3: Verify Session Type

```bash
echo $XDG_SESSION_TYPE
# Should output: wayland
```

### Step 4: Check Graphics

```bash
# Check graphics hardware
lspci | grep -i vga
# Should show: Intel Corporation Raptor Lake-P [Iris Xe Graphics]

# Check loaded graphics module
lsmod | grep i915
# Should show i915 module loaded

# Check for errors
dmesg | grep -i "drm\|i915" | tail -20
```

---

## Troubleshooting

### If login still loops:

1. **Try X11 session instead**:
   - At SDDM login screen, click session dropdown (bottom left)
   - Select "Plasma (X11)" instead of "Plasma (Wayland)"
   - Try logging in

2. **Check logs from TTY**:
   - Press `Ctrl+Alt+F3` to get to TTY3
   - Login as user
   - Run: `journalctl -b -p err`
   - Run: `journalctl -b | grep -i "kwin\|plasma\|sddm" | tail -50`

3. **Restore KDE config backup**:
   ```bash
   cd ~
   tar -xzf kde-config-backup-20251225.tar.gz
   ```

---

## What We Did NOT Do

- ❌ Did NOT install X11 session (as requested)
- ❌ Did NOT change display manager (kept SDDM)
- ❌ Did NOT reinstall Plasma
- ❌ Did NOT modify Ubuntu installation

---

## Files Modified

- `/etc/sddm.conf.d/` - No changes (removed temporary X11 config)
- `/home/torstein.sornes/.config/plasma*` - Removed
- `/home/torstein.sornes/.config/kwin*` - Removed  
- `/home/torstein.sornes/.local/share/kwin` - Removed
- `/etc/group` - Added user to video and render groups
- `/boot/initramfs-*.img` - Regenerated all initramfs images
- All system files - Permissions/ownership restored via RPM

---

## Backup Files Created

- `/home/torstein.sornes/kde-config-backup-20251225.tar.gz` - KDE configuration backup

---

## Hardware Info

- **GPU**: Intel Iris Xe Graphics (Raptor Lake-P)
- **Driver**: i915 (kernel module)
- **Mesa**: 25.1.9-1.fc42
- **Kernel**: 6.17.11, 6.17.12, 6.17.13
- **Display Server**: SDDM
- **Desktop**: KDE Plasma 6.x
- **Session Type**: Wayland (default)

---

## Next Steps After Successful Login

Once Fedora login is working, you can proceed with the partition resize:

1. **Phase 1 (from Ubuntu)**: Shrink Fedora LUKS/btrfs by 50GB
2. **Phase 2 (from Fedora)**: Move and extend Ubuntu partition

See `DUAL_BOOT_RESIZE_GUIDE.md` for detailed instructions.

---

**Status**: Fixes complete, ready for reboot and testing