#!/usr/bin/env python

# Collect speed data from a driving ICE/IC train.

import requests

from gps3 import gps3 as gps

import argparse
import json
import sys
import time

def eprint(*args):
    print(*args, file=sys.stderr)

def send_point(sess, args, info: dict[str, str]):
    geohub_templ = args.geohub + '/{CLIENT}/log?secret={SECRET}'
    geohub_url = geohub_templ.format(HOST=args.geohub_host, CLIENT=args.client or info.get('tzn', 'TRAIN'), SECRET=args.secret, PROTOCOL=args.geohub_scheme)
    additional = '&lat={lat}&longitude={long}&s={spd}&time={ts}'.format(
            lat=info['lat'], long=info['lon'], spd=info['speed'], ts=info['time'])
    # Delete unnecessary data.
    url = geohub_url + additional
    return sess.post(url, data="")

def fetch_data(datastream, data, seen=set()):
    if data is None:
        return None
    datastream.unpack(data)
    if datastream.TPV['time'] in seen:
        return None
    seen.add(datastream.TPV['time'])
    return datastream.TPV

def parse_args():
    parser = argparse.ArgumentParser(description='Fetch and send gpsd data')
    parser.add_argument('--client', default='gpsd', help='Client name.')
    parser.add_argument('--secret', default='', help='Secret. This protects your current location; to share it, you have to share the secret. By default, the points will be made public on your GeoHub instance.')
    parser.add_argument('--interval', default=5, type=int, help='Poll interval. If 0, send every point received from gpsd.')
    parser.add_argument('--outfile', default='data.jsonlines', help='Where to write the JSON data received from the train.')
    parser.add_argument('--geohub_host', default='example.com', help='Host of your GeoHub. Use this if the URL --geohub works for you.')
    parser.add_argument('--geohub_scheme', default='https', help='Protocol scheme of the GeoHub instance. Use this if you do not want to specify the entire --geohub URL')
    parser.add_argument('--geohub', default='{PROTOCOL}://{HOST}/geo/', help='Base URL of Geohub instance. E.g., https://example.com/geo. Use --geohub_host, --geohub_scheme if your URL looks like the example.')
    return parser.parse_args()

def run(args):
    session = requests.Session()
    socket = gps.GPSDSocket()
    datastream = gps.DataStream()
    socket.connect()
    socket.watch()

    geohub_base = args.geohub.format(PROTOCOL=args.geohub_scheme, HOST=args.geohub_host)
    livemap_url = geohub_base + 'assets/livemap.html?client={client}&secret={secret}'.format(client=args.client, secret=args.secret)
    eprint('Go to LiveMap:', livemap_url);

    seen = set()
    last_time = time.time()

    with open(args.outfile, 'w') as outfile:
        while True:
            data = socket.next(timeout=5)
            if data:
                info = fetch_data(datastream, data, seen=seen)
                if time.time() - last_time < args.interval:
                    continue
                elif info is not None:
                    last_time = time.time()
                    eprint('{} :: Sending point ({}, {}) to GeoHub.'.format(info['time'], info['lon'], info['lat']))
                    send_point(session, args, info)
                    outfile.write(json.dumps(info))
                    outfile.write('\n')
            # Prevent memory leak.
            if len(seen) > 100:
                seen = set()

def main():
    args = parse_args()
    run(args)


if __name__ == '__main__':
    main()
