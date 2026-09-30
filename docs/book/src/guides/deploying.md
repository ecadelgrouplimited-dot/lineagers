# Deploying the guard server

The guard server is a single static binary with its state in one directory. A production deployment needs four things: a service manager, TLS in front of it, a managed admin token, and backups.

## 1. Install

```sh
curl -fsSL https://lineagrs.tech/install.sh | sudo LINEAGE_INSTALL_DIR=/usr/local/bin sh
sudo useradd --system --home /var/lib/lineage-guard --create-home lineage-guard
```

## 2. Configure

```sh
sudo install -d -m 0700 /etc/lineage-guard
echo "GUARD_ADMIN_TOKEN=$(openssl rand -hex 32)" | sudo tee /etc/lineage-guard/env >/dev/null
sudo chmod 0400 /etc/lineage-guard/env
```

systemd reads this file as root before starting the service, so it can stay readable by root only.

| Variable | Default | |
|---|---|---|
| `GUARD_ADMIN_TOKEN` | read from, or created in, `<data dir>/keys/admin.token` | Operator token, 32+ characters |
| `GUARD_DATA_DIR` | `./guard-data` | Keys and logs |
| `GUARD_BIND` | `127.0.0.1:9200` | Listen address. Keep it on localhost behind a proxy |

## 3. Run under systemd

`/etc/systemd/system/lineage-guard.service`:

```ini
[Unit]
Description=Lineage guard server
After=network.target

[Service]
User=lineage-guard
EnvironmentFile=/etc/lineage-guard/env
Environment=GUARD_DATA_DIR=/var/lib/lineage-guard/data
Environment=GUARD_BIND=127.0.0.1:9200
ExecStart=/usr/local/bin/guard-server
Restart=on-failure
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/lineage-guard
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now lineage-guard
journalctl -u lineage-guard -f
```

Run **one** server per data directory. Log files are locked against a second writer, so a second server fails to open them.

## 4. TLS with nginx

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name guard.example.com;

    ssl_certificate     /etc/letsencrypt/live/guard.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/guard.example.com/privkey.pem;

    client_max_body_size 2m;

    location / {
        proxy_pass http://127.0.0.1:9200;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

Consider putting the console (`/`) and admin endpoints behind your VPN or SSO, and exposing only what agents need.

## Docker instead

```sh
docker build -f apps/guard-server/Dockerfile -t lineage-guard .
docker run -d --name lineage-guard --restart unless-stopped \
  -p 127.0.0.1:9200:9200 \
  --env-file /etc/lineage-guard/env \
  -v lineage-guard-data:/data lineage-guard
```

## Backups

Back up the whole data directory: `keys/` and `agents/`. Logs are append-only, so incremental backups work well. Treat `keys/` like any other secret store: encrypted backups and restricted access.

Losing `audit.key` doesn't lose history: logs still verify with the public key. But they can't be appended to, so their agents can't act again.

## Monitoring

- **Liveness:** `GET /healthz` returns `{"ok": true}`.
- **Quarantine:** the startup output reports `N quarantined`. Any agent whose log failed verification shows as `quarantined` in `GET /v1/agents` and the console; investigate those as incidents.
- **Checkpoints:** periodically call `GET /v1/agents/:id/verify`, and store the returned `head` somewhere the guard host can't modify. That's what makes truncation and rewrites detectable later.
