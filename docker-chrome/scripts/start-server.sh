#!/bin/bash
cd /opt/server
nohup node index.js > /var/log/node-server.log 2>&1 &
