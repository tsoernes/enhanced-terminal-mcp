# Dual-Boot Resize Guide: Shrink from Ubuntu, Move/Extend from Fedora

## Strategy Overview

This approach uses **both operating systems** to safely resize partitions:
- **Phase 1 (Ubuntu)**: Shrink the Fedora LUKS/btrfs filesystem
- **Phase 2 (Fedora)**: Shrink the Fedora partition, move Ubuntu partition, extend Ubuntu

This is safer because we work on unmounted partitions from the "other" OS!

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
4. **Allow 3-4 hours total** - Don't start if you need your laptop soon
5. **Follow the order exactly** - Don't skip steps!

---

## PHASE 1: Shrink Fedora Filesystem (From Ubuntu)

### Step 1: Backup (Do This First!)

```bash
# Create partition table backup
sudo sgdisk --backup=$HOME/partition-backup.sgd /dev/nvme0n1
sudo sfdisk -d /dev/nvme0n1 > $HOME/partition-backup.txt

# Verify backups exist
ls -lh $HOME/partition-backup.*

# COPY THESE TO EXTERNAL DRIVE OR CLOUD!
cp $HOME/partition-backup.* /path/to/external/drive/
```

### Step 2: Unlock and Mount Fedora Partition

```bash
# Unlock LUKS (you'll be prompted for passphrase)
sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora_resize

# Verify it's unlocked
ls -l /dev/mapper/fedora_resize

# Mount the Fedora partition
sudo mkdir -p /mnt/fedora
sudo mount /dev/mapper/fedora_resize /mnt/fedora

# Verify mount and check usage
df -h /mnt/fedora
sudo btrfs filesystem usage /mnt/fedora
```

Expected output should show:
- Device size: ~448.72GiB
- Used: ~339.55GiB
- Free: ~103.34GiB

### Step 3: Balance and Defragment Btrfs (Optional but Recommended)

This consolidates data and makes shrinking safer:

```bash
# Balance to consolidate data (this may take 30-60 minutes)
sudo btrfs balance start -dusage=75 /mnt/fedora

# Check progress if needed
sudo btrfs balance status /mnt/fedora

# After balance completes, defragment
sudo btrfs filesystem defragment -r -v /mnt/fedora
```

**Note**: You can skip this if you're in a hurry, but it's recommended.

### Step 4: Shrink Btrfs Filesystem by 50GB

```bash
# Check current size
sudo btrfs filesystem show /mnt/fedora

# Shrink by 50GB (this should be quick - a few seconds)
sudo btrfs filesystem resize -50G /mnt/fedora

# Verify new size
sudo btrfs filesystem show /mnt/fedora
sudo btrfs filesystem usage /mnt/fedora
```

Expected result:
- Device size: ~398.72GiB (was 448.72GiB)
- Used: ~339.55GiB (unchanged)
- Free: ~53.34GiB (reduced from 103.34GiB)

### Step 5: Shrink LUKS Container

Now we need to shrink the LUKS container to match the filesystem:

```bash
# Calculate the new size in sectors
# 398.7GB ≈ 428032409600 bytes ≈ 836000800 sectors (512 bytes each)
# Use 398GB to be safe: 427737899008 bytes = 835620896 sectors

# Check current LUKS size
sudo cryptsetup status fedora_resize

# Resize LUKS container (this should be quick)
sudo cryptsetup resize --size 835620896 fedora_resize

# Verify
sudo cryptsetup status fedora_resize
```

### Step 6: Verify Everything

```bash
# Check filesystem is still healthy
sudo btrfs filesystem show /mnt/fedora
df -h /mnt/fedora

# List some files to ensure data is accessible
ls -la /mnt/fedora/home
ls -la /mnt/fedora/etc

# Unmount
sudo umount /mnt/fedora

# Close LUKS
sudo cryptsetup luksClose fedora_resize

# Verify it closed
ls /dev/mapper/fedora_resize  # Should say "No such file"
```

### Step 7: Create Verification File

Create a file to remind yourself what to do next:

```bash
cat > $HOME/resize-phase2-instructions.txt << 'EOF'
PHASE 1 COMPLETE - Fedora filesystem and LUKS shrunk by 50GB

NEXT STEPS:
1. Reboot into Fedora
2. Open terminal in Fedora
3. Run the commands from PHASE 2 of the guide
4. DO NOT run any Fedora updates or make changes to the partition yet!

Partition state:
- Fedora BTRFS: ~399GB (shrunk)
- Fedora PARTITION (nvme0n1p3): Still 448.7GB (needs shrinking)
- Ubuntu PARTITION (nvme0n1p4): Still 26.6GB (needs moving/extending)
EOF

cat $HOME/resize-phase2-instructions.txt
```

**IMPORTANT**: Phase 1 is now complete. The Fedora **filesystem** is shrunk, but the **partition** is still the old size. We'll fix that in Phase 2 from Fedora.

---

## PHASE 2: Shrink Partition, Move & Extend Ubuntu (From Fedora)

### Step 1: Reboot into Fedora

```bash
# From Ubuntu, reboot
sudo reboot
```

At the GRUB menu, select Fedora.

### Step 2: Verify Ubuntu is NOT Mounted

```bash
# Check that Ubuntu partition is NOT mounted
lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT | grep nvme0n1p4

# If it shows a mountpoint, unmount it
sudo umount /dev/nvme0n1p4

# Verify it's unmounted
lsblk | grep nvme0n1p4
```

### Step 3: Install Required Tools (if needed)

```bash
# Check if parted is installed
which parted

# Install if needed
sudo dnf install -y parted btrfs-progs

# Also install gpart for safety
sudo dnf install -y gpart
```

### Step 4: Verify Current Partition Layout

```bash
# Show current partitions
sudo fdisk -l /dev/nvme0n1

# Should show:
# /dev/nvme0n1p3   3328000  944388095  941060096  448.7G  Linux filesystem (LUKS)
# /dev/nvme0n1p4 944388096 1000212479  55824384  26.6G  Linux filesystem (btrfs)

# Show in human-readable format
lsblk -o NAME,SIZE,START,END,TYPE,FSTYPE
```

### Step 5: Backup Partition Table (Again, from Fedora)

```bash
# Backup from Fedora too (extra safety)
sudo sgdisk --backup=$HOME/partition-backup-fedora.sgd /dev/nvme0n1
sudo sfdisk -d /dev/nvme0n1 > $HOME/partition-backup-fedora.txt

# Copy to external drive
cp $HOME/partition-backup-fedora.* /run/media/$(whoami)/YOUR_EXTERNAL_DRIVE/
```

### Step 6: Shrink Fedora Partition (nvme0n1p3)

**CRITICAL**: We're now modifying the partition table. This is the risky part!

```bash
# Use parted to resize partition 3
sudo parted /dev/nvme0n1

# In parted prompt:
(parted) print
# Note the current end sector of partition 3: 944388095s

(parted) resizepart 3
# You'll be prompted: "End?"
# Calculate new end: start (3328000) + new size in sectors
# 398GB = 427737899008 bytes ÷ 512 = 835620896 sectors
# New end: 3328000 + 835620896 = 838948896

End? [944388095s]? 838948896s

(parted) print
# Verify partition 3 now ends at ~838948896

(parted) quit
```

**Alternative calculation** (if you prefer GiB units):
```bash
sudo parted /dev/nvme0n1
(parted) unit GiB
(parted) print
(parted) resizepart 3
End? [448.7GiB]? 398.7GiB
(parted) print
(parted) quit
```

### Step 7: Verify Partition Resize

```bash
# Check new partition layout
sudo fdisk -l /dev/nvme0n1

# Should now show:
# /dev/nvme0n1p3   3328000  838948896  835620897  398.7G
# /dev/nvme0n1p4 944388096 1000212479  55824384  26.6G

# Notice the gap between p3 end (838948896) and p4 start (944388096)
# Gap = 944388096 - 838948896 = 105439200 sectors ≈ 50GB
```

### Step 8: Delete and Recreate Ubuntu Partition

**WARNING**: This doesn't delete data, just the partition entry!

```bash
# First, create an image backup of Ubuntu partition (OPTIONAL but SAFE)
# This takes ~15-30 minutes but provides safety
sudo partclone.btrfs -c -s /dev/nvme0n1p4 -o /tmp/ubuntu-backup.img

# Alternatively, use dd (faster but less safe)
sudo dd if=/dev/nvme0n1p4 of=/tmp/ubuntu-backup.img bs=4M status=progress

# Now delete and recreate partition 4
sudo parted /dev/nvme0n1

(parted) print
# Note current p4: start=944388096, end=1000212479

(parted) rm 4
# Partition 4 is now deleted (data still on disk!)

(parted) mkpart primary btrfs 838948897s 1000212479s
# Creates new partition 4 starting right after p3
# Uses same end sector as before (for now)

(parted) print
# Should show new p4: start=838948897, end=1000212479

(parted) quit
```

### Step 9: Inform Kernel of Partition Changes

```bash
# Tell kernel to re-read partition table
sudo partprobe /dev/nvme0n1

# Verify kernel sees new layout
cat /proc/partitions | grep nvme0n1
```

### Step 10: Extend Ubuntu Partition to End of Disk

```bash
# Now extend partition 4 to use the full disk
sudo parted /dev/nvme0n1

(parted) print
# Current p4: 838948897s to 1000212479s (still small)

(parted) resizepart 4
End? [1000212479s]? 100%
# Or manually: 1000215215s (last sector on disk)

(parted) print
# Should now show p4 using all space to end of disk
# New size should be ~77GB

(parted) quit
```

### Step 11: Verify Final Partition Layout

```bash
# Check final layout
sudo fdisk -l /dev/nvme0n1

# Should show:
# /dev/nvme0n1p3   3328000  838948896  835620897  398.7G  (Fedora)
# /dev/nvme0n1p4 838948897 1000215215 161266319   76.9G  (Ubuntu)

lsblk -o NAME,SIZE,TYPE,FSTYPE
```

### Step 12: Extend Ubuntu Btrfs Filesystem

The partition is bigger, but the filesystem inside still thinks it's small:

```bash
# Mount Ubuntu partition
sudo mkdir -p /mnt/ubuntu
sudo mount /dev/nvme0n1p4 /mnt/ubuntu

# Check current filesystem size (should still show ~27GB)
df -h /mnt/ubuntu

# Extend btrfs to fill the partition
sudo btrfs filesystem resize max /mnt/ubuntu

# Verify new size (should show ~77GB)
df -h /mnt/ubuntu
sudo btrfs filesystem show /mnt/ubuntu

# List files to verify data is intact
ls -la /mnt/ubuntu/home

# Unmount
sudo umount /mnt/ubuntu
```

### Step 13: Final Verification

```bash
# Run filesystem check on Ubuntu partition (read-only)
sudo btrfs check --readonly /dev/nvme0n1p4

# Should complete with no errors

# Check Fedora partition still works
df -h /

# Verify partition table is consistent
sudo sgdisk -v /dev/nvme0n1
```

### Step 14: Create Success Marker

```bash
cat > $HOME/resize-complete.txt << 'EOF'
PARTITION RESIZE COMPLETE!

Final layout:
- Fedora (nvme0n1p3): ~399GB
- Ubuntu (nvme0n1p4): ~77GB

Next step:
- Reboot into Ubuntu
- Verify Ubuntu shows ~77GB available
- Verify Fedora still boots and works
- Keep backups for 1 week just in case

Date completed: $(date)
EOF

cat $HOME/resize-complete.txt
```

---

## PHASE 3: Final Verification (From Ubuntu)

### Step 1: Reboot into Ubuntu

```bash
# From Fedora
sudo reboot
```

Select Ubuntu from GRUB menu.

### Step 2: Verify Ubuntu Filesystem

```bash
# Check filesystem size (should show ~77GB)
df -h /

# Check partition layout
lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT

# Verify no errors in system log
sudo dmesg | grep -i error
sudo journalctl -p err -b
```

### Step 3: Verify Fedora Still Works

```bash
# Mount Fedora to verify it's intact
sudo mkdir -p /mnt/fedora
sudo cryptsetup luksOpen /dev/nvme0n1p3 luks-5410cf79-bdca-4c37-b640-91c389f40461
sudo mount /dev/mapper/luks-5410cf79-bdca-4c37-b640-91c389f40461 /mnt/fedora

# Check Fedora filesystem
df -h /mnt/fedora
ls -la /mnt/fedora/home

# Verify btrfs is healthy
sudo btrfs filesystem usage /mnt/fedora

# Unmount
sudo umount /mnt/fedora
sudo cryptsetup luksClose luks-5410cf79-bdca-4c37-b640-91c389f40461
```

### Step 4: Test Fedora Boot (Optional but Recommended)

```bash
# Reboot and try booting into Fedora
sudo reboot
```

At GRUB, select Fedora and verify it boots normally.

---

## Success Criteria

✅ Ubuntu shows ~77GB total space (was ~27GB)
✅ Fedora shows ~399GB total space (was ~449GB)
✅ Both operating systems boot normally
✅ No filesystem errors in logs
✅ All files accessible in both systems

---

## Troubleshooting

### Problem: "btrfs resize failed: No space left on device"

**From Ubuntu (Phase 1):**
```bash
# Balance more aggressively before shrinking
sudo btrfs balance start -dusage=50 /mnt/fedora
sudo btrfs balance start -musage=50 /mnt/fedora
sudo btrfs filesystem resize -50G /mnt/fedora
```

### Problem: "cryptsetup resize failed"

```bash
# Check LUKS status
sudo cryptsetup status fedora_resize

# Try without explicit size (auto-detect from filesystem)
sudo cryptsetup resize fedora_resize

# Or try with different size calculation
# Be conservative - use 395GB instead of 398GB
# 395GB = 424674197504 bytes = 829441792 sectors
sudo cryptsetup resize --size 829441792 fedora_resize
```

### Problem: "parted won't let me shrink partition"

```bash
# Make sure LUKS is closed first
sudo cryptsetup luksClose fedora_resize

# Then try parted again
sudo parted /dev/nvme0n1
(parted) resizepart 3 398.7GiB
```

### Problem: "Ubuntu partition shows wrong size after extend"

```bash
# Mount and manually resize
sudo mount /dev/nvme0n1p4 /mnt/ubuntu
sudo btrfs filesystem resize max /mnt/ubuntu
df -h /mnt/ubuntu
sudo umount /mnt/ubuntu
```

### Problem: "Partition table corrupted"

```bash
# Restore from backup (choose the most recent)
sudo sgdisk --load-backup=/path/to/partition-backup.sgd /dev/nvme0n1

# Or from Fedora backup
sudo sgdisk --load-backup=/path/to/partition-backup-fedora.sgd /dev/nvme0n1

# Then reboot and try again
```

### Problem: "Ubuntu won't boot after resize"

Boot from live USB and repair:
```bash
sudo btrfs check --repair /dev/nvme0n1p4
sudo mount /dev/nvme0n1p4 /mnt
sudo grub-install --boot-directory=/mnt/boot /dev/nvme0n1
sudo update-grub
```

### Problem: "Fedora won't boot after resize"

Boot from Fedora live USB and repair:
```bash
sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora_repair
sudo btrfs check --repair /dev/mapper/fedora_repair
sudo mount /dev/mapper/fedora_repair /mnt
sudo grub2-install --boot-directory=/mnt/boot /dev/nvme0n1
sudo grub2-mkconfig -o /mnt/boot/grub2/grub.cfg
```

---

## Time Estimates

**Phase 1 (Ubuntu):**
- Balance/defrag: 30-60 minutes (optional)
- Shrink btrfs: 1-2 minutes
- Shrink LUKS: 1 minute
- **Total: 5-60 minutes**

**Phase 2 (Fedora):**
- Partition operations: 5-10 minutes
- Extend filesystem: 1-2 minutes
- **Total: 10-15 minutes**

**Phase 3 (Verification):**
- 5-10 minutes

**Grand Total: 20-85 minutes** (depending on whether you balance/defrag)

---

## Why This Approach is Better

✅ **Safer**: Work on unmounted partitions from the other OS
✅ **No live USB needed**: Use your existing dual-boot setup
✅ **Faster**: No need to move data physically (partition table changes only)
✅ **Easier to recover**: If something fails, just boot the other OS
✅ **Less risky**: Filesystem shrink happens first (most reversible step)

---

## Important Notes

1. **Keep backups for at least 1 week** after completing the resize
2. **Don't delete the backup files** until you're 100% sure everything works
3. **Test both OS boots** multiple times in the first week
4. **Monitor disk space** - Fedora now has less space, so watch usage
5. **Keep laptop plugged in** throughout the entire process

---

**Good luck! This approach is much safer than using live USB and moving data around!**