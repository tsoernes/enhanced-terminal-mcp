# Partition Resize Guide: Shrink Fedora 50GB, Extend Ubuntu

## Current Situation
- **Fedora partition (nvme0n1p3)**: 448.7G (341G used, 103G free) - LUKS encrypted btrfs
- **Ubuntu partition (nvme0n1p4)**: 26.6G (**100% full!** - CRITICAL)
- Partition order: EFI -> Boot -> Fedora (p3) -> Ubuntu (p4)

## Plan
1. Boot into Fedora (so Ubuntu partition is not mounted)
2. Shrink Fedora LUKS partition from 448.7G to 398.7G (freeing 50GB)
3. Move Ubuntu partition left by 50GB
4. Extend Ubuntu partition to reclaim the 50GB at the end
5. Final result: Fedora ~399G, Ubuntu ~77G

## ⚠️ CRITICAL WARNINGS
- **BACKUP ALL IMPORTANT DATA BEFORE PROCEEDING**
- This operation is RISKY - power loss or errors can cause data loss
- Keep your laptop plugged in - DO NOT let battery die
- Have your LUKS passphrase ready for the Fedora partition
- The entire process may take 2-4 hours due to moving data

## Step 1: Backup (Run on Ubuntu NOW)

```bash
# Backup partition table
sudo sgdisk --backup=$HOME/partition-backup.sgd /dev/nvme0n1
sudo sfdisk -d /dev/nvme0n1 > $HOME/partition-backup.txt

# Copy to external drive or cloud storage!
# Backup any critical data from Ubuntu (it's 100% full!)
```

## Step 2: Boot into Fedora

1. Reboot your system
2. Select Fedora from the GRUB menu
3. Log in

## Step 3: Verify Ubuntu is Not Mounted

```bash
# Check that Ubuntu partition is NOT mounted
lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT,LABEL | grep nvme0n1p4

# If it shows a mountpoint, unmount it:
sudo umount /dev/nvme0n1p4
```

## Step 4: Install Required Tools (if needed)

```bash
# On Fedora
sudo dnf install -y btrfs-progs cryptsetup parted gpart
```

## Step 5: Shrink Fedora Btrfs Filesystem

The Fedora filesystem is currently mounted as your root. We need to shrink it while running.

```bash
# Check current btrfs usage
sudo btrfs filesystem usage /

# Check current size
sudo btrfs filesystem show /

# Shrink btrfs filesystem by 50GB
# Current: ~449GB, Target: ~399GB
sudo btrfs filesystem resize -50G /

# Verify the new size
sudo btrfs filesystem show /
sudo btrfs filesystem usage /
```

## Step 6: Shrink LUKS Container

Now we need to shrink the LUKS container to match the smaller btrfs filesystem.

```bash
# The btrfs is now ~399GB
# Calculate sectors: 399GB = 428899696640 bytes = 837890032 sectors (512-byte)
# But safer to use: 398GB = 427737899008 bytes = 835620896 sectors

# First, get current LUKS size
sudo cryptsetup status luks-5410cf79-bdca-4c37-b640-91c389f40461

# Resize LUKS container to 398GB (835620896 sectors of 512 bytes)
# This is done ONLINE while the system is running
sudo cryptsetup resize --size 835620896 luks-5410cf79-bdca-4c37-b640-91c389f40461
```

## Step 7: Reboot to Live Environment

Now we need to work on the actual partition table and move partitions. This CANNOT be done while Fedora is running since we need to resize nvme0n1p3.

**Option A: Use Ubuntu Live USB**
1. Create Ubuntu 24.04 Live USB
2. Boot from USB
3. Choose "Try Ubuntu"

**Option B: Use SystemRescue or GParted Live**
1. Download SystemRescue ISO
2. Create bootable USB
3. Boot from it

Continue with remaining steps from live environment...

## Step 8: Shrink Partition p3 (From Live Environment)

```bash
# Verify Ubuntu partition is not mounted
lsblk | grep nvme0n1p4

# Check current partition layout
sudo fdisk -l /dev/nvme0n1

# Note: 
# Current p3: sectors 3328000 to 944388095 (941060096 sectors = 448.7GB)
# Target p3: 398GB = 835620896 sectors
# New p3 end: 3328000 + 835620896 = 838948896

# Use parted to resize partition 3
sudo parted /dev/nvme0n1

# In parted:
(parted) print
(parted) resizepart 3
End? [944388095s]? 838948896s
(parted) print
(parted) quit
```

## Step 9: Move Ubuntu Partition Left

```bash
# Current p4 starts at: 944388096
# New p4 should start at: 838948897 (right after p3)
# This creates ~50GB of free space at the end

# Use dd to move the partition data (CAREFUL!)
# First, calculate the offset
# Old start: 944388096, New start: 838948897
# Offset: 944388096 - 838948897 = 105439199 sectors = 53984869888 bytes

# Copy partition data to new location
sudo dd if=/dev/nvme0n1 of=/dev/nvme0n1 \
  skip=944388096 seek=838948897 \
  bs=1M count=27264 \
  conv=notrunc,noerror status=progress

# OR use partclone (safer):
# Create image of Ubuntu partition
sudo partclone.btrfs -c -s /dev/nvme0n1p4 -o /tmp/ubuntu.img

# Delete partition 4
sudo parted /dev/nvme0n1 rm 4

# Recreate partition 4 at new location
# New start: 838948897
# New end: use rest of disk (1000212479)
sudo parted /dev/nvme0n1
(parted) mkpart primary btrfs 838948897s 1000212479s
(parted) print
(parted) quit

# Restore the image
sudo partclone.btrfs -r -s /tmp/ubuntu.img -o /dev/nvme0n1p4
```

## Step 10: Extend Ubuntu Btrfs Filesystem

```bash
# Mount the Ubuntu partition
sudo mkdir -p /mnt/ubuntu
sudo mount /dev/nvme0n1p4 /mnt/ubuntu

# Extend btrfs to fill the partition
sudo btrfs filesystem resize max /mnt/ubuntu

# Verify
sudo btrfs filesystem show /mnt/ubuntu
df -h /mnt/ubuntu

# Unmount
sudo umount /mnt/ubuntu
```

## Step 11: Verify Everything

```bash
# Check partition table
sudo fdisk -l /dev/nvme0n1

# Should show:
# p3: ~398GB (Fedora)
# p4: ~77GB (Ubuntu)

# Test LUKS opening
sudo cryptsetup luksOpen /dev/nvme0n1p3 test-fedora
ls /dev/mapper/test-fedora
sudo cryptsetup luksClose test-fedora

# Test Ubuntu mount
sudo mount /dev/nvme0n1p4 /mnt/ubuntu
ls /mnt/ubuntu
sudo umount /mnt/ubuntu
```

## Step 12: Reboot into Ubuntu

```bash
sudo reboot
```

## Step 13: Post-Reboot Verification (In Ubuntu)

```bash
# Check filesystem size
df -h /

# Should show ~77GB total instead of ~27GB

# Verify partition layout
lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT

# Check that Fedora still works
sudo mkdir -p /mnt/fedora
sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora
sudo mount /dev/mapper/fedora /mnt/fedora
ls /mnt/fedora
df -h /mnt/fedora
sudo umount /mnt/fedora
sudo cryptsetup luksClose fedora
```

## EASIER ALTERNATIVE: Use GParted GUI

Instead of manual dd/parted commands, use GParted from live USB:

```bash
# Boot into live USB
# Open terminal
sudo gparted /dev/nvme0n1

# In GParted GUI:
# 1. Right-click /dev/nvme0n1p3 -> Resize/Move
#    - Set new size: 398 GiB
#    - Click Resize/Move
# 
# 2. Right-click /dev/nvme0n1p4 -> Resize/Move
#    - Drag partition LEFT to eliminate free space before it
#    - Drag right edge RIGHT to extend to end of disk
#    - Click Resize/Move
#
# 3. Review operations queue
# 4. Click Apply (✓) button
# 5. WAIT 2-4 hours for completion
# 6. Reboot
```

## Troubleshooting

### If btrfs resize fails:
```bash
# Balance filesystem first
sudo btrfs balance start -dusage=75 /
sudo btrfs filesystem resize -50G /
```

### If LUKS resize fails:
```bash
# Check LUKS status
sudo cryptsetup status luks-5410cf79-bdca-4c37-b640-91c389f40461

# Try without size parameter (auto-detect)
sudo cryptsetup resize luks-5410cf79-bdca-4c37-b640-91c389f40461
```

### If partition move fails:
Use GParted instead - it's much more reliable for moving partitions.

## Recovery (If Things Go Wrong)

```bash
# Boot from live USB
# Restore partition table
sudo sgdisk --load-backup=/path/to/partition-backup.sgd /dev/nvme0n1

# Check filesystems
sudo btrfs check --readonly /dev/nvme0n1p4
sudo cryptsetup luksOpen /dev/nvme0n1p3 fedora
sudo btrfs check --readonly /dev/mapper/fedora
```

## Time Estimate
- Btrfs shrink: 10-20 minutes
- LUKS resize: 5 minutes
- Partition move (GParted): 2-3 hours
- Filesystem extend: 5 minutes
- **Total: 2.5-4 hours**

## Expected Final Result
- Fedora: 398.7GB (341GB used, 57GB free)
- Ubuntu: 76.6GB (27GB used, 49GB free)