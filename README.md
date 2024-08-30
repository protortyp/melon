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

1. Install tools to `/usr/local/bin`

   ```
   sudo install-all.sh
   ```

2. Set up the worker (see [Setting up the Worker](#setting-up-the-worker-cgroups-permissions))

3. Start the daemon:

   ```
   melond --port 8081
   ```

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
Environment=MELOND_ENDPOINT=http://[::1]:8080
Environment=MWORKER_PORT=8082
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
