#!/usr/bin/env python

# Collect speed data from a driving ICE/IC train.

import requests

import argparse
import json
import sys
import time

def eprint(*args):
    print(*args, file=sys.stderr)

def fetch_current(api):
    return requests.get(api).json()

def format_server_time(servertime):
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(servertime/1000))

def send_point(sess, args, info: dict[str, str]):
    geohub_templ = args.geohub + '/{CLIENT}/log?secret={SECRET}'
    geohub_url = geohub_templ.format(HOST=args.geohub_host, CLIENT=args.client or info.get('tzn', 'TRAIN'), SECRET=args.secret, PROTOCOL=args.geohub_scheme)
    additional = '&lat={lat}&longitude={long}&s={spd}&time={ts}'.format(
            lat=info['latitude'], long=info['longitude'], spd=info['speed'], ts=format_server_time(info['serverTime']))
    # Delete unnecessary data.
    for k in ['latitude', 'longitude', 'speed', 'serverTime']:
        info.pop(k)
    url = geohub_url + additional
    sess.post(url, json=info)

def parse_args():
    parser = argparse.ArgumentParser(description='Fetch and send train data')
    parser.add_argument('--api', default='https://iceportal.de/api1/rs/status', help='Location of train API')
    parser.add_argument('--client', default='', help='Client name. By default, this will be the `tzn` (train number) of the current train.')
    parser.add_argument('--secret', default='', help='Secret. This protects your current location; to share it, you have to share the secret. By default, the points will be made public on your GeoHub instance.')
    parser.add_argument('--interval', default=5, type=int, help='Poll interval')
    parser.add_argument('--outfile', default='traindata.jsonlines', help='Where to write the JSON data received from the train.')
    parser.add_argument('--geohub_host', default='example.com', help='Host of your GeoHub. Use this if the URL --geohub works for you.')
    parser.add_argument('--geohub_scheme', default='https', help='Protocol scheme of the GeoHub instance. Use this if you do not want to specify the entire --geohub URL')
    parser.add_argument('--geohub', default='{PROTOCOL}://{HOST}/geo/', help='Base URL of Geohub instance. E.g., https://example.com/geo. Use --geohub_host, --geohub_scheme if your URL looks like the example.')
    return parser.parse_args()

def run(args):
    info = fetch_current(args.api)
    if not info:
        eprint('Empty info received!')
        return
    tzn = info['tzn']
    geohub_base = args.geohub.format(PROTOCOL=args.geohub_scheme, HOST=args.geohub_host)
    livemap_url = geohub_base + 'assets/livemap.html?client={client}&secret={secret}'.format(client=args.client, secret=args.secret)
    eprint('Running in train:', tzn)
    eprint('Go to LiveMap:', livemap_url);

    session = requests.Session()

    lastpoint = None

    with open(args.outfile, 'w') as outfile:
        while True:
            info = fetch_current(args.api)
            if lastpoint is None or lastpoint != (info['latitude'], info['longitude']):
                lastpoint = (info['latitude'], info['longitude'])
                if info:
                    eprint('{} :: Sending point ({}, {}) to GeoHub.'.format(format_server_time(info['serverTime']), info['longitude'], info['latitude']))
                    send_point(session, args, info)
                    outfile.write(json.dumps(info))
                    outfile.write('\n')
                else:
                    eprint('{} :: Skipped point due to no API response.'.format(format_server_time(time.time_ns()/1e6)))
            else:
                eprint('{} :: Skipped duplicate point.'.format(format_server_time(time.time_ns()/1e6)))
            time.sleep(args.interval)

def main():
    args = parse_args()
    run(args)


if __name__ == '__main__':
    main()
