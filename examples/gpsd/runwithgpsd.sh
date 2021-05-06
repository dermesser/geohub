#!/bin/bash

# Example: start gpsd along the gpsd.py script, point the latter at the former,
# and wait for NMEA data on UDP port 10001.

PORT=2948

UDPADDRESS=10.0.1.1
UDPPORT=10001

gpsd -N udp://${UDPADDRESS}:${UDPPORT} -P /tmp/gpsd${PORT}.pid -S ${PORT}  &

python gpsd.py \
    --geohub_host 127.0.0.80 \
    --geohub_scheme http \
    --client lbogpsd  \
    --secret 29482948 \
    --interval 0 \
    --gpsdport ${PORT}
