#!/bin/bash

# create new user
sudo adduser mworker --no-create-home --disabled-login

# set up cgroup delegation
sudo mkdir /sys/fs/cgroup/melon
sudo chown -R mworker:mworker /sys/fs/cgroup/melon
echo "+cpu +memory +io" | sudo tee /sys/fs/cgroup/melon/cgroup.subtree_control

# set sudo permissions
echo "mworker ALL=(root) NOPASSWD: /bin/echo [0-9]* > /sys/fs/cgroup/melon/*/cgroup.procs" | sudo EDITOR='tee -a' visudo
echo "mworker ALL=(root) NOPASSWD: /bin/mkdir /sys/fs/cgroup/melon/*" | sudo EDITOR='tee -a' visudo
echo "mworker ALL=(root) NOPASSWD: /bin/echo \"+*\" > /sys/fs/cgroup/melon/*/cgroup.subtree_control" | sudo EDITOR='tee -a' visudo
