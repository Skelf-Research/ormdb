# Deployment Guide

Deploy ORMDB in various environments.

## Deployment Options

| Option | Best For | Complexity |
|--------|----------|------------|
| Docker | Development, small production | Low |
| Docker Compose | Multi-service setups | Low |
| Kubernetes | Scalable production | Medium |
| Bare metal | Maximum performance | High |

## Docker Deployment

### Basic Docker Run

```bash
docker run -d \
  --name ormdb \
  -p 8080:8080 \
  -v ormdb-data:/data \
  ormdb/ormdb:latest
```

### With Configuration

```bash
docker run -d \
  --name ormdb \
  -p 8080:8080 \
  -p 9090:9090 \
  -v ormdb-data:/data \
  -v ./ormdb.toml:/etc/ormdb/ormdb.toml:ro \
  -e ORMDB_STORAGE_CACHE_SIZE_MB=512 \
  ormdb/ormdb:latest
```

### Docker Compose

```yaml
# docker-compose.yml
version: "3.8"

services:
  ormdb:
    image: ormdb/ormdb:latest
    ports:
      - "8080:8080"
      - "9090:9090"
    volumes:
      - ormdb-data:/data
      - ./ormdb.toml:/etc/ormdb/ormdb.toml:ro
      - ./schema.json:/etc/ormdb/schema.json:ro
    environment:
      - ORMDB_STORAGE_PATH=/data
      - ORMDB_STORAGE_CACHE_SIZE_MB=512
      - ORMDB_LOGGING_LEVEL=info
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 10s
      timeout: 5s
      retries: 5
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: "2"
    restart: unless-stopped

volumes:
  ormdb-data:
    driver: local
```

## Kubernetes Deployment

### Deployment Manifest

```yaml
# ormdb-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ormdb
  labels:
    app: ormdb
spec:
  replicas: 1
  selector:
    matchLabels:
      app: ormdb
  template:
    metadata:
      labels:
        app: ormdb
    spec:
      containers:
        - name: ormdb
          image: ormdb/ormdb:latest
          ports:
            - containerPort: 8080
              name: http
            - containerPort: 9090
              name: metrics
          volumeMounts:
            - name: data
              mountPath: /data
            - name: config
              mountPath: /etc/ormdb
              readOnly: true
          env:
            - name: ORMDB_STORAGE_PATH
              value: /data
            - name: ORMDB_STORAGE_CACHE_SIZE_MB
              value: "1024"
          resources:
            requests:
              memory: "1Gi"
              cpu: "500m"
            limits:
              memory: "4Gi"
              cpu: "2"
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /ready
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 5
      volumes:
        - name: data
          persistentVolumeClaim:
            claimName: ormdb-data
        - name: config
          configMap:
            name: ormdb-config
```

### Service

```yaml
# ormdb-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: ormdb
  labels:
    app: ormdb
spec:
  type: ClusterIP
  ports:
    - port: 8080
      targetPort: http
      name: http
    - port: 9090
      targetPort: metrics
      name: metrics
  selector:
    app: ormdb
```

### PersistentVolumeClaim

```yaml
# ormdb-pvc.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ormdb-data
spec:
  accessModes:
    - ReadWriteOnce
  storageClassName: fast-ssd
  resources:
    requests:
      storage: 100Gi
```

### ConfigMap

```yaml
# ormdb-configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: ormdb-config
data:
  ormdb.toml: |
    [server]
    host = "0.0.0.0"
    port = 8080

    [storage]
    path = "/data"
    cache_size_mb = 1024
    sync_mode = "normal"

    [query]
    max_entities = 10000
    max_depth = 5

    [logging]
    level = "info"
    format = "json"

    [metrics]
    enabled = true
    port = 9090
```

### Helm Chart (Coming Soon)

```bash
# Add ORMDB Helm repository
helm repo add ormdb https://charts.skelfresearch.com/ormdb
helm repo update

# Install ORMDB
helm install my-ormdb ormdb/ormdb \
  --set storage.size=100Gi \
  --set resources.memory=4Gi
```

## High Availability

### Replication Setup

ORMDB supports primary-replica replication:

```yaml
# Primary
services:
  ormdb-primary:
    image: ormdb/ormdb:latest
    environment:
      - ORMDB_REPLICATION_ROLE=primary
      - ORMDB_REPLICATION_BIND=0.0.0.0:5433
    ports:
      - "8080:8080"
      - "5433:5433"

  # Replica
  ormdb-replica:
    image: ormdb/ormdb:latest
    environment:
      - ORMDB_REPLICATION_ROLE=replica
      - ORMDB_REPLICATION_PRIMARY=ormdb-primary:5433
    ports:
      - "8081:8080"
    depends_on:
      - ormdb-primary
```

### Load Balancer Configuration

```yaml
# HAProxy example
frontend ormdb_frontend
    bind *:8080
    mode http
    default_backend ormdb_backend

backend ormdb_backend
    mode http
    balance roundrobin
    option httpchk GET /health
    server ormdb1 ormdb-1:8080 check
    server ormdb2 ormdb-2:8080 check
    server ormdb3 ormdb-3:8080 check
```

## TLS Configuration

### Using Certificates

```toml
# ormdb.toml
[server]
tls_cert = "/etc/ormdb/tls/cert.pem"
tls_key = "/etc/ormdb/tls/key.pem"
```

### Using Reverse Proxy (Recommended)

```nginx
# nginx.conf
upstream ormdb {
    server localhost:8080;
}

server {
    listen 443 ssl http2;
    server_name db.example.com;

    ssl_certificate /etc/nginx/ssl/cert.pem;
    ssl_certificate_key /etc/nginx/ssl/key.pem;

    location / {
        proxy_pass http://ormdb;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## Cloud-Specific Deployments

### AWS ECS

```json
{
  "family": "ormdb",
  "containerDefinitions": [
    {
      "name": "ormdb",
      "image": "ormdb/ormdb:latest",
      "portMappings": [
        {"containerPort": 8080, "protocol": "tcp"},
        {"containerPort": 9090, "protocol": "tcp"}
      ],
      "mountPoints": [
        {
          "sourceVolume": "ormdb-data",
          "containerPath": "/data"
        }
      ],
      "environment": [
        {"name": "ORMDB_STORAGE_PATH", "value": "/data"},
        {"name": "ORMDB_STORAGE_CACHE_SIZE_MB", "value": "1024"}
      ],
      "memory": 4096,
      "cpu": 2048
    }
  ],
  "volumes": [
    {
      "name": "ormdb-data",
      "efsVolumeConfiguration": {
        "fileSystemId": "fs-12345678"
      }
    }
  ]
}
```

### Google Cloud Run

```yaml
# service.yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: ormdb
spec:
  template:
    spec:
      containers:
        - image: ormdb/ormdb:latest
          ports:
            - containerPort: 8080
          env:
            - name: ORMDB_STORAGE_PATH
              value: /data
          volumeMounts:
            - name: data
              mountPath: /data
          resources:
            limits:
              memory: 4Gi
              cpu: "2"
      volumes:
        - name: data
          persistentVolumeClaim:
            claimName: ormdb-data
```

## Deployment Best Practices

### 1. Use Persistent Storage

Always use persistent volumes for data:

```yaml
volumes:
  - name: data
    persistentVolumeClaim:
      claimName: ormdb-data  # Not emptyDir!
```

### 2. Set Resource Limits

Prevent resource exhaustion:

```yaml
resources:
  requests:
    memory: "1Gi"
    cpu: "500m"
  limits:
    memory: "4Gi"
    cpu: "2"
```

### 3. Configure Health Checks

Ensure proper service discovery:

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 8080
  initialDelaySeconds: 10

readinessProbe:
  httpGet:
    path: /ready
    port: 8080
  initialDelaySeconds: 5
```

### 4. Use Rolling Updates

Zero-downtime deployments:

```yaml
spec:
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
```

### 5. Separate Data and Logs

```yaml
volumeMounts:
  - name: data
    mountPath: /data
  - name: logs
    mountPath: /var/log/ormdb
```

## Upgrade Procedures

### Rolling Upgrade

```bash
# 1. Update image tag
kubectl set image deployment/ormdb ormdb=ormdb/ormdb:v2.0.0

# 2. Monitor rollout
kubectl rollout status deployment/ormdb

# 3. Rollback if needed
kubectl rollout undo deployment/ormdb
```

### Blue-Green Deployment

```bash
# 1. Deploy new version alongside old
kubectl apply -f ormdb-v2-deployment.yaml

# 2. Test new version
curl http://ormdb-v2:8080/health

# 3. Switch traffic
kubectl patch service ormdb -p '{"spec":{"selector":{"version":"v2"}}}'

# 4. Remove old version
kubectl delete deployment ormdb-v1
```

---

## Next Steps

- **[Monitoring](monitoring.md)** - Set up observability
- **[Backup & Restore](backup-restore.md)** - Protect your data
- **[Configuration Reference](../reference/configuration.md)** - All configuration options
