#!/bin/bash
#MBATCH -c 4
#MBATCH -t 0-06:00
#MBATCH -m 1G
#MBATCH --account=test_account
#MBATCH -o logs/basic_script_%j.out
#MBATCH -e logs/basic_script_%j.err


for i in $(seq 1 100); do
  sleep 1
  echo $i
done

echo "super basic test"
