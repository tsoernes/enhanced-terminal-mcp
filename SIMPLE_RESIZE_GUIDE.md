# Simple Partition Resize Guide: Shrink Fedora 50GB, Extend Ubuntu

## Current Situation
- **Fedora (nvme0n1p3)**: 448.7GB (341GB used, 103GB free) - LUKS encrypted btrfs
- **Ubuntu (nvme0n1p4)**: 26.6GB (**100% FULL!**)
- Goal: Shrink Fedora by 50GB, give that space to Ubuntu

## Final Result
- **Fedora**: ~399GB (still plenty of space)
- **Ubuntu**: ~77GB (almost 3x current size)

---

## ⚠️ CRITICAL WARNINGS

1. **BACKUP YOUR DATA** - This operation can cause data loss if interrupted
2. **Keep laptop plugged in** - Battery death during operation = disaster
3. **Have LUKS passphrase ready** - You'll need it to unlock Fedora partition
4. **Allow 2-4 hours** - Don't start this if you need your laptop soon
5. **Do NOT interrupt** - Let GParted finish completely

---

## Step 1: Backup (Do This NOW in Ubuntu)

```bash
# Create partition table backup
sudo sgdisk --backup=$HOME/partition-backup.sgd /dev/nvme0n1
sudo sfdisk -d /dev/nvme0n1 > $HOME/partition-backup.txt

# Display backup files
ls -lh $HOME/partition-backup.*

# COPY THESE FILES TO EXTERNAL DRIVE OR CLOUD!
# Also backup any critical data from Ubuntu
```

---

## Step 2: Download and Create GParted Live USB

### Download GParted Live
1. Go to: https://gparted.org/download.php
2. Download the ISO (gparted-live-X.X.X-X-amd64.iso)

### Create Bootable USB (on Ubuntu)
```bash
# Insert USB drive (at least 1GB)
# Find USB device name
lsblk

# Assuming USB is /dev/sdb (VERIFY THIS!)
# WARNING: This will erase everything on the USB!
sudo dd if=/path/to/gparted-live-*.iso of=/dev/sdb bs=4M status=progress
sync
```

**OR use Balena Etcher / Startup Disk Creator GUI**

---

## Step 3: Boot from GParted Live USB

1. **Insert the USB drive**
2. **Reboot computer**
3. **Press F12** during Dell logo to open boot menu
4. **Select USB drive** from boot menu
5. **Wait for GParted Live to boot** (choose default options)
6. **Select language and keymap** when prompted

---

## Step 4: Unlock LUKS Partition

Before GParted can work with the encrypted Fedora partition, we need to unlock it:

```bash
# Open terminal in GParted Live (icon on desktop or menu)

# Unlock LUKS partition (you'll be prompted for passphrase)
sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora_temp

# Verify it's unlocked
ls /dev/mapper/fedora_temp

# Mount it to verify access
sudo mkdir /mnt/fedora
sudo mount /dev/mapper/fedora_temp /mnt/fedora
ls /mnt/fedora

# Check usage
df -h /mnt/fedora
```

**Important**: Keep this terminal open! Don't close the LUKS device yet.

---

## Step 5: Open GParted and Resize Partitions

```bash
# Launch GParted (from terminal or menu)
sudo gparted /dev/nvme0n1
```

### In GParted GUI:

#### A. Shrink Fedora (nvme0n1p3)

1. You should see `/dev/mapper/fedora_temp` in the device dropdown
2. Select it from the dropdown menu
3. Right-click on the large partition (should show btrfs filesystem)
4. Click **Resize/Move**
5. In the dialog:
   - **New size**: 398 GiB (or type 341 + 57 = 398)
   - Or: Drag the RIGHT edge LEFT to shrink by ~50GB
   - Make sure "Free space following" shows ~50GB
6. Click **Resize/Move** button
7. **DON'T CLICK APPLY YET!**

#### B. Move Ubuntu Partition Left (nvme0n1p4)

1. Switch back to `/dev/nvme0n1` in the device dropdown (top)
2. You should now see ~50GB of unallocated space between p3 and p4
3. Right-click on **nvme0n1p4** (Ubuntu partition)
4. Click **Resize/Move**
5. In the dialog:
   - **Drag the entire partition LEFT** to eliminate the free space before it
   - The partition should now start immediately after p3
   - You should see ~50GB of free space AFTER p4 now
6. Click **Resize/Move** button
7. **DON'T CLICK APPLY YET!**

#### C. Extend Ubuntu Partition (nvme0n1p4)

1. Right-click on **nvme0n1p4** again
2. Click **Resize/Move**
3. In the dialog:
   - **Drag the RIGHT edge RIGHT** to extend to end of disk
   - Or set "Free space following" to 0
   - New size should be ~77GB
4. Click **Resize/Move** button
5. **DON'T CLICK APPLY YET!**

#### D. Review and Apply All Operations

1. Look at the **operations queue** at the bottom of GParted
2. You should see 3 operations:
   - Shrink /dev/mapper/fedora_temp (or similar)
   - Move nvme0n1p4
   - Resize nvme0n1p4
3. **CAREFULLY REVIEW** - make sure they look correct
4. Click the **green checkmark (Apply)** button
5. Click **Apply** in the confirmation dialog
6. **WAIT** - This will take 2-4 hours (mostly the move operation)
7. Do NOT interrupt, close laptop, or let battery die!

---

## Step 6: After GParted Finishes

```bash
# In the terminal where you unlocked LUKS:

# Unmount Fedora
sudo umount /mnt/fedora

# Close LUKS device
sudo cryptsetup luksClose fedora_temp

# Verify partition table
sudo fdisk -l /dev/nvme0n1

# Should show:
# nvme0n1p3: ~398GB (Fedora)
# nvme0n1p4: ~77GB (Ubuntu)
```

---

## Step 7: Reboot into Ubuntu

```bash
# Remove USB drive
# Reboot
sudo reboot
```

Select Ubuntu from GRUB menu.

---

## Step 8: Verify Everything Works (In Ubuntu)

```bash
# Check Ubuntu filesystem size
df -h /
# Should show ~77GB total (up from ~27GB)

# Check partition layout
lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT

# Verify Fedora still works
sudo mkdir -p /mnt/fedora
sudo cryptsetup luksOpen /dev/nvme0n1p3 luks-5410cf79-bdca-4c37-b640-91c389f40461
sudo mount /dev/mapper/luks-5410cf79-bdca-4c37-b640-91c389f40461 /mnt/fedora
ls /mnt/fedora
df -h /mnt/fedora
sudo umount /mnt/fedora
sudo cryptsetup luksClose luks-5410cf79-bdca-4c37-b640-91c389f40461
```

If everything looks good - **SUCCESS!** 🎉

---

## Troubleshooting

### GParted shows partition as busy/locked
- Make sure you're booted from live USB, not from installed OS
- Unmount any mounted partitions before resizing

### Can't unlock LUKS partition
```bash
# Check partition exists
ls -l /dev/nvme0n1p3

# Try with full UUID
sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora_temp --verbose
```

### GParted operation fails
- Don't panic!
- GParted usually rolls back changes automatically
- Restore partition table from backup:
  ```bash
  sudo sgdisk --load-backup=/path/to/partition-backup.sgd /dev/nvme0n1
  ```

### After reboot, Ubuntu won't boot
- Boot from live USB again
- Run filesystem check:
  ```bash
  sudo btrfs check --readonly /dev/nvme0n1p4
  sudo btrfs check --repair /dev/nvme0n1p4  # only if errors found
  ```

### After reboot, Fedora won't boot
- Boot from live USB
- Check LUKS and btrfs:
  ```bash
  sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora_check
  sudo btrfs check --readonly /dev/mapper/fedora_check
  sudo btrfs check --repair /dev/mapper/fedora_check  # only if errors found
  ```

---

## Quick Reference: Expected Values

| Partition | Before | After |
|-----------|--------|-------|
| nvme0n1p3 (Fedora) | 448.7GB | ~399GB |
| nvme0n1p4 (Ubuntu) | 26.6GB | ~77GB |

**Total time**: 2-4 hours (mostly waiting for partition move)

**Difficulty**: Medium (mostly automated by GParted)

**Risk level**: Medium (backup required, but GParted is reliable)

---

## Pro Tips

1. **Do this overnight** - You can start the process and let it run while you sleep
2. **Use a UPS** if available - Protects against power failures
3. **Test Fedora mounting** before rebooting - Makes sure encryption still works
4. **Keep the backup files** - Store them safely for future emergencies

---

## Alternative: SystemRescue Instead of GParted Live

SystemRescue includes GParted plus more recovery tools:

1. Download from: https://www.system-rescue.org/
2. Create bootable USB same way as GParted
3. Boot from it, select "Boot SystemRescue using default options"
4. Once booted, run: `sudo gparted /dev/nvme0n1`
5. Follow same steps as above

---

**Good luck! Remember: Backup first, keep plugged in, don't interrupt!**