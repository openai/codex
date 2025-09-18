#!/bin/bash

echo "$ALLOWED_IPS_CSV" | tr ',' '\n' | while read -r ip
do
    if [ -n "$ip" ]; then
        # allow ssh access from the ip
        ufw allow from "$ip" to any port 22
    fi
done
