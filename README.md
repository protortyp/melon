# 🍉 Melon

[![codecov](https://codecov.io/github/protortyp/melon/graph/badge.svg?token=O0IPLF902F)](https://codecov.io/github/protortyp/melon)
[![dependency status](https://deps.rs/repo/github/protortyp/melon/status.svg)](https://deps.rs/repo/github/protortyp/melon)

Melon is a lightweight distributed job scheduler written in Rust, inspired by Slurm but with added job extension capabilities.

## Key Features

- Job submission and management
- Resource allocation and limitations (using cgroups on Linux)
- Job extension functionality
- Simple web UI for job monitoring

## Quick Start

0. Get the repo

   ```bash
   git clone git@github.com:protortyp/melon.git
   cd melon
   ```

1. Install tools to `/usr/local/bin` (will ask for `sudo` access to move the tools to `/usr/local/bin/`)

   ```
   install.sh
   ```

2. Set up the scheduler daemon (see [Setting up the scheduler](#setting-up-the-scheduler))

3. Set up the worker (see [Setting up the Worker](#setting-up-the-worker-cgroups-permissions))

4. Submit a job:

   ```bash
   echo '#!/bin/bash
   #MBATCH -c 4
   #MBATCH -t 0-06:00
   #MBATCH -m 1G

   echo "Hello, Melon!"
   ' > job.sh

   mbatch job.sh
   ```

5. Manage jobs:
   - List jobs: `mqueue`
   - Extend job time: `mextend $JOBID -t 1-00-00`
   - Cancel job: `mcancel $JOBID`
   - Show job details: `mshow $JOBID` or `mshow $JOBID -p` for json output

## Setting up the Scheduler

Create a new user `melond`:

```bash
sudo adduser melond --no-create-home --disabled-login
```

Create the configuration file with your preferred settings:

```bash
sudo mkdir /var/lib/melon
sudo chown -R melond:melond /var/lib/melon
sudo cp crates/melond/configuration/base.yaml /var/lib/melon
sudo tee /var/lib/melon/production.yaml > /dev/null << EOF
application:
  port: 8080
  host: "127.0.0.1"
database:
  path: "/var/lib/melon/melon.sqlite"
api:
  port: 8088
  host: "127.0.0.1"
EOF
```

Then, create a new file `/etc/systemd/system/melond.service` with the following content.

```
[Unit]
Description=Melon Scheduler
After=network.target

[Service]
Environment=APP_ENVIRONMENT=production
Environment=RUST_LOG=info
Environment="CONFIG_PATH=/var/lib/melon"
ExecStart=/usr/local/bin/melond
User=melond
Group=melond
Restart=always

[Install]
WantedBy=multi-user.target
```

Start and enable the scheduler:

```bash
sudo systemctl daemon-reload
sudo systemctl start melond
sudo systemctl enable melond
```

## Setting up the Worker Cgroups Permissions

Run the setup script using sudo:

```bash
sudo bash setup_mworker.sh
```

Then, create a new file `/etc/systemd/system/mworker.service` with the following content:

```
[Unit]
Description=Melon Worker
After=network.target

[Service]
Environment=MELOND_ENDPOINT=127.0.0.1:8080
Environment=MWORKER_PORT=8082
Environment=APP_ENVIRONMENT=production
Environment=RUST_LOG=info
Environment="CONFIG_PATH=/path/to/config/folder"
ExecStart=/usr/local/bin/mworker --api_endpoint ${MELOND_ENDPOINT} --port ${MWORKER_PORT}
User=mworker
Group=mworker
Restart=always

[Install]
WantedBy=multi-user.target
```

Start and enable the Melon worker:

```bash
sudo systemctl daemon-reload
sudo systemctl start mworker
sudo systemctl enable mworker
```

You can check the status of the service with:

```
sudo systemctl status mworker
```
