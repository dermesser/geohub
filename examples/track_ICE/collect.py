#!/usr/bin/env python

# Collect speed data from a driving ICE/IC train.

import requests

import argparse
import json
import sys
import time

def fetch_current(api):
    return requests.get(api).json()

def format_server_time(servertime):
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(servertime/1000))

def send_point(args, info):
    geohub_url = args.geohub.format(HOST=args.geohub_host, CLIENT=args.client or info.get('tzn', 'TRAIN'), SECRET=args.secret, PROTOCOL=args.geohub_scheme)
    additional = '&lat={lat}&longitude={long}&spd={spd}&time={ts}'.format(
            lat=info['latitude'], long=info['longitude'], spd=info['speed'], ts=format_server_time(info['serverTime']))
    url = geohub_url + additional
    requests.post(url, json=info)

def parse_args():
    parser = argparse.ArgumentParser(description='Fetch and send train data')
    parser.add_argument('--api', default='https://iceportal.de/api1/rs/status', help='Location of train API')
    parser.add_argument('--client', default='', help='Client name. By default, this will be the `tzn` (train number) of the current train.')
    parser.add_argument('--secret', default='', help='Secret. This protects your current location; to share it, you have to share the secret. By default, the points will be made public on your GeoHub instance.')
    parser.add_argument('--interval', default=5, help='Poll interval')
    parser.add_argument('--outfile', default='traindata.jsonlines', help='Where to write the JSON data received from the train.')
    parser.add_argument('--geohub_host', default='example.com', help='Host of your GeoHub. Use this if the URL --geohub works for you.')
    parser.add_argument('--geohub_scheme', default='https', help='Protocol scheme of the GeoHub instance. Use this if you do not want to specify the entire --geohub URL')
    parser.add_argument('--geohub', default='{PROTOCOL}://{HOST}/geo/{CLIENT}/log?secret={SECRET}', help='Base URL of Geohub instance. {PROTOCOL}, {CLIENT}, {HOST}, and {SECRET} will be replaced by the --geohub_scheme, --client, --geohub_host, and --secret values, respectively. This string must end in the URL query parameter section, ready to take more parameters.')

    return parser.parse_args()

def run(args):
    info = fetch_current(args.api)
    if not info:
        print('Empty info received!')
        return
    tzn = info['tzn']
    print('Running in train:', tzn)

    with open(args.outfile, 'w') as outfile:
        while True:
            send_point(args, info)
            outfile.write(json.dumps(info))
            outfile.write('\n')
            time.sleep(args.interval)

def main():
    args = parse_args()
    run(args)


if __name__ == '__main__':
    main()
