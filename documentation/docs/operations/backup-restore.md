# Backup and Restore Guide

Protect your data with ORMDB backup strategies.

## Overview

ORMDB supports multiple backup methods:

| Method | Use Case | Recovery Time | Data Loss |
|--------|----------|---------------|-----------|
| Full backup | Disaster recovery | Minutes | Point-in-time |
| Incremental | Frequent backups | Minutes | Minimal |
| Continuous | Real-time protection | Seconds | Near-zero |
| Snapshot | Quick restore points | Seconds | Point-in-time |

## Full Backup

### Create Full Backup

```bash
# Basic backup
ormdb backup create backup.ormdb

# Compressed backup
ormdb backup create backup.ormdb.gz --compress

# With progress
ormdb backup create backup.ormdb --progress

# To remote storage
ormdb backup create s3://bucket/backups/backup-$(date +%Y%m%d).ormdb
```

### Backup Options

| Option | Description |
|--------|-------------|
| `--compress` | Gzip compression |
| `--encrypt` | Encrypt with key |
| `--parallel N` | Parallel threads |
| `--exclude-entity` | Skip entities |

### Example with All Options

```bash
ormdb backup create \
  s3://my-bucket/backups/full-$(date +%Y%m%d-%H%M%S).ormdb.gz \
  --compress \
  --encrypt --key-file /etc/ormdb/backup.key \
  --parallel 4 \
  --exclude-entity SessionLog \
  --exclude-entity TempData
```

## Incremental Backup

### Enable WAL Archiving

```toml
# ormdb.toml
[backup]
wal_archive_enabled = true
wal_archive_path = "/var/lib/ormdb/wal-archive"
# Or remote
wal_archive_command = "aws s3 cp %f s3://bucket/wal/%f"
```

### Create Incremental Backup

```bash
# First full backup
ormdb backup create --full backup-base.ormdb

# Subsequent incremental backups
ormdb backup create --incremental backup-incr-001.ormdb
ormdb backup create --incremental backup-incr-002.ormdb
```

### Restore from Incremental

```bash
# Restore base + apply incrementals
ormdb backup restore backup-base.ormdb \
  --apply-wal /var/lib/ormdb/wal-archive \
  --target-time "2024-01-15 12:00:00"
```

## Continuous Backup (CDC-Based)

### Stream to External Storage

```bash
# Stream changes to S3
ormdb backup stream \
  --destination s3://bucket/cdc/ \
  --format parquet \
  --partition-by day
```

### Restore from Stream

```bash
# Replay from CDC stream
ormdb backup restore-stream \
  --source s3://bucket/cdc/ \
  --target ./restored-data \
  --until "2024-01-15T12:00:00Z"
```

## Storage Snapshots

### Filesystem Snapshot

```bash
# Pause writes (brief)
ormdb admin pause-writes

# Take filesystem snapshot
lvcreate -L 10G -s -n ormdb-snap /dev/vg0/ormdb-data

# Resume writes
ormdb admin resume-writes

# Mount and copy snapshot
mount /dev/vg0/ormdb-snap /mnt/snapshot
cp -r /mnt/snapshot /backup/ormdb-$(date +%Y%m%d)
umount /mnt/snapshot
lvremove /dev/vg0/ormdb-snap
```

### Cloud Snapshots

```bash
# AWS EBS
aws ec2 create-snapshot \
  --volume-id vol-1234567890abcdef0 \
  --description "ORMDB backup $(date +%Y%m%d)"

# GCP
gcloud compute disks snapshot ormdb-disk \
  --snapshot-names=ormdb-backup-$(date +%Y%m%d)

# Azure
az snapshot create \
  --resource-group mygroup \
  --source ormdb-disk \
  --name ormdb-backup-$(date +%Y%m%d)
```

## Restore Procedures

### Basic Restore

```bash
# Stop server
ormdb server stop

# Restore backup
ormdb backup restore backup.ormdb --target /var/lib/ormdb/data

# Verify integrity
ormdb admin verify --data-dir /var/lib/ormdb/data

# Start server
ormdb server start
```

### Point-in-Time Recovery

```bash
# Restore to specific time
ormdb backup restore backup.ormdb \
  --target /var/lib/ormdb/data \
  --apply-wal /var/lib/ormdb/wal-archive \
  --target-time "2024-01-15 12:00:00 UTC"
```

### Restore to Different Location

```bash
# Restore to new directory
ormdb backup restore backup.ormdb --target /data/ormdb-restored

# Start with different config
ormdb server start \
  --data-dir /data/ormdb-restored \
  --port 8081
```

### Partial Restore

```bash
# Restore specific entities only
ormdb backup restore backup.ormdb \
  --target /data/ormdb-partial \
  --include-entity User \
  --include-entity Post
```

## Backup Verification

### Verify Backup Integrity

```bash
# Check backup file
ormdb backup verify backup.ormdb

# Output:
# Backup verification
# ─────────────────────
# File: backup.ormdb
# Size: 5.4 GB
# Created: 2024-01-15 12:00:00
# Checksum: SHA256:abc123...
# Status: Valid
```

### Test Restore

```bash
# Restore to temporary location
ormdb backup restore backup.ormdb --target /tmp/ormdb-test

# Start test server
ormdb server start --data-dir /tmp/ormdb-test --port 8081

# Run verification queries
ormdb query User --limit 10 --port 8081

# Cleanup
ormdb server stop --port 8081
rm -rf /tmp/ormdb-test
```

## Backup Automation

### Cron Schedule

```bash
# /etc/cron.d/ormdb-backup
# Daily full backup at 2 AM
0 2 * * * ormdb /usr/bin/ormdb backup create /backup/daily/ormdb-$(date +\%Y\%m\%d).ormdb.gz --compress

# Hourly incremental
0 * * * * ormdb /usr/bin/ormdb backup create --incremental /backup/hourly/ormdb-incr-$(date +\%Y\%m\%d-\%H).ormdb
```

### Backup Script

```bash
#!/bin/bash
# /usr/local/bin/ormdb-backup.sh

set -e

BACKUP_DIR="/backup/ormdb"
S3_BUCKET="s3://my-bucket/ormdb-backups"
RETENTION_DAYS=30

# Create backup
BACKUP_FILE="$BACKUP_DIR/ormdb-$(date +%Y%m%d-%H%M%S).ormdb.gz"
ormdb backup create "$BACKUP_FILE" --compress

# Upload to S3
aws s3 cp "$BACKUP_FILE" "$S3_BUCKET/"

# Verify upload
aws s3 ls "$S3_BUCKET/$(basename $BACKUP_FILE)"

# Cleanup old local backups
find "$BACKUP_DIR" -name "*.ormdb.gz" -mtime +7 -delete

# Cleanup old S3 backups
aws s3 ls "$S3_BUCKET/" | while read -r line; do
  created=$(echo $line | awk '{print $1}')
  filename=$(echo $line | awk '{print $4}')
  created_ts=$(date -d "$created" +%s)
  cutoff_ts=$(date -d "-$RETENTION_DAYS days" +%s)
  if [ $created_ts -lt $cutoff_ts ]; then
    aws s3 rm "$S3_BUCKET/$filename"
  fi
done

echo "Backup completed: $BACKUP_FILE"
```

### Kubernetes CronJob

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: ormdb-backup
spec:
  schedule: "0 2 * * *"
  jobTemplate:
    spec:
      template:
        spec:
          containers:
            - name: backup
              image: ormdb/ormdb:latest
              command:
                - /bin/sh
                - -c
                - |
                  ormdb backup create /backup/ormdb-$(date +%Y%m%d).ormdb.gz \
                    --compress \
                    --host ormdb-service:8080
                  aws s3 cp /backup/*.ormdb.gz s3://bucket/backups/
              volumeMounts:
                - name: backup
                  mountPath: /backup
              env:
                - name: AWS_ACCESS_KEY_ID
                  valueFrom:
                    secretKeyRef:
                      name: aws-credentials
                      key: access-key
                - name: AWS_SECRET_ACCESS_KEY
                  valueFrom:
                    secretKeyRef:
                      name: aws-credentials
                      key: secret-key
          volumes:
            - name: backup
              emptyDir: {}
          restartPolicy: OnFailure
```

## Disaster Recovery

### Recovery Plan

1. **Assess the situation**
   - Identify cause of failure
   - Determine data loss extent
   - Choose recovery method

2. **Prepare recovery environment**
   - Provision new server if needed
   - Ensure network connectivity
   - Verify backup accessibility

3. **Execute recovery**
   ```bash
   # Download latest backup
   aws s3 cp s3://bucket/backups/latest.ormdb.gz /tmp/

   # Restore
   ormdb backup restore /tmp/latest.ormdb.gz \
     --target /var/lib/ormdb/data

   # Start server
   ormdb server start
   ```

4. **Verify recovery**
   ```bash
   # Check data integrity
   ormdb admin verify

   # Verify entity counts
   ormdb aggregate User count
   ormdb aggregate Post count
   ```

5. **Update DNS/routing**
   - Point applications to recovered server

### Recovery Time Objectives

| Scenario | RTO | RPO |
|----------|-----|-----|
| Single disk failure | < 1 hour | Near-zero (with replication) |
| Server failure | < 2 hours | Last backup |
| Data center failure | < 4 hours | Last off-site backup |
| Regional disaster | < 8 hours | Last cross-region backup |

## Best Practices

### 1. Follow 3-2-1 Rule

- **3** copies of data
- **2** different storage types
- **1** off-site location

### 2. Test Restores Regularly

```bash
# Monthly restore test
ormdb backup restore latest.ormdb --target /tmp/test-restore
ormdb admin verify --data-dir /tmp/test-restore
```

### 3. Monitor Backup Jobs

```yaml
# Alert on backup failure
- alert: BackupFailed
  expr: |
    time() - ormdb_backup_last_success_timestamp > 86400
  for: 1h
  labels:
    severity: critical
```

### 4. Encrypt Sensitive Backups

```bash
ormdb backup create backup.ormdb.enc \
  --encrypt \
  --key-file /etc/ormdb/backup.key
```

### 5. Document Recovery Procedures

Maintain runbooks with:
- Step-by-step recovery instructions
- Contact information
- Access credentials (securely stored)
- Expected recovery times

---

## Next Steps

- **[Troubleshooting](troubleshooting.md)** - Recovery problem solving
- **[Monitoring](monitoring.md)** - Monitor backup jobs
- **[Deployment](deployment.md)** - High availability setup
