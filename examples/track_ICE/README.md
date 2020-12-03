# `track_ICE`

The German IC (InterCity) and ICE (InterCityExpress) trains have -- mostly --
on-board Wi-Fi with entertainment and internet uplink.

As any good nerd, we are not interested in some fringe German TV shows. We find
that to implement the live map on the website, DB has thankfully built a little
API:

* `GET https://iceportal.de/api1/rs/status`
  * Returns a JSON object. For example, in the last car of an ICE2:

```json
{
  "connection": true,
  "servicelevel": "AVAILABLE_SERVICE",
  "internet": "HIGH",
  "speed": 159,
  "gpsStatus": "VALID",
  "tzn": "Tz208",
  "series": "807",
  "latitude": 52.601715,
  "longitude": 12.369854,
  "serverTime": 1605641662281,
  "wagonClass": "SECOND",
  "navigationChange": "2020-11-17-16-04-20",
  "trainType": "ICE"
}

```

or a middle car of a doubledecker IC2:

```json
{
  "connection": true,
  "servicelevel": "AVAILABLE_SERVICE",
  "internet": "MIDDLE",
  "speed": 72,
  "gpsStatus": "VALID",
  "tzn": "ICD2871",
  "series": "100",
  "latitude": 50.96594,
  "longitude": 7.019422,
  "serverTime": 1606577933620,
  "wagonClass": "SECOND",
  "navigationChange": "2020-11-28-06-25-31",
  "trainType": "IC"
}
```

We can now directly store and evaluate position and speed. And because we are
interested in the speed and internet quality of our train to evaluate at home,
let's send them as metadata along the coordinates.

The enclosed python script does just that. Once run, it will

* Fetch the current train data at a regular interval (5 seconds)
* Log it to a file consisting of JSON objects separated by newlines
* Send it to a configured URL.

Run it with `--help` to obtain more info. Typically, you might run:

```bash
$ ./collect.py --geohub_host=my.geohub.com --client=trainjourney007
```

which will log the current location along with the complete train info returned
by the API (see above) as note to the specified GeoHub.
