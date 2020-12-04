# `geohub`

## What is GeoHub?

GeoHub is -- in fancy terms -- a framework for real time geographic
applications. It currently allows for

* Ingesting geographic points, for example GPS positions, via HTTP API.
* Exporting geographic points as GeoJSON (GPX soon to come).
* Efficiently listening to position updates (new points) in real time via HTTP
API.
* Privacy first by protecting every point with a password (session token).

## API description

Important concepts are:

* A **Client** is for example someone walking around with their phone, or a
moving car. A client sends geographic updates to the server. The `client`
appears as URL component after `/geo/`.
* A **Secret** is an alphanumeric string (a-zA-Z0-9 only) attached to every
point. Anyone wanting to retrieve a given point needs to know its secret. You
can also log points without secret. In that case, everyone can see them
(including clients asking for points with a secret). The `secret` is supplied as
URL parameter `&secret=`.
* Every point has a unique integer **ID**. It can be used to limit which points
to fetch, for example when refreshing or waiting on updates. Think of it as a
very fine-grained page token.

*Scenario*: You go for a walk, configuring your phone to send live updates to a
GeoHub instance. You want to share it with a friend who is not supposed to know
about where you were yesterday. You can leave your `client` string the same and
use a new `secret` that you give to your friend; they can now only see points
logged with this secret.

* `POST` `/geo/<client>/log?lat=<latitude>&longitude=<longitude>&time=<time>&s=<speed>&ele=<elevation>&secret=<secret>`
  * Log a new point.
  * `latitude`, `longitude`: Geographical position, in decimal degrees (note:
      may be extended later). **Required**.
  * `time`: ISO 8601 time. If left out, current server time is used. Example:
  `2020-12-03T15:42:40.010325Z`. **Optional**.
  * `speed`: Speed in km/h (usually). If you decide to always use m/s, you are
  free to do so. **Optional**.
  * `elevation`: Elevation in meters. **Optional**.
  * A body -- if present and encoded in whatever content-type -- is attached as `note` to the
  point and returned as property `note` of GeoJSON points later.
  * Usually returns code **200** except for server errors (500) or malformed inputs (400).
* `GET` `/geo/assets/...`
  * Static file serving. The `assets` directory should be deployed in the
  current working directory from which the server is run.
* `GET` `/geo/<client>/retrieve/json?secret=<secret>&from=<from_timestamp>&to=<to_timestamp>&limit=<maximum
number of entries returned>&last=<id of last known entry>`
  * Fetch geo data as GeoJSON object.
  * `from`, `to`: Timestamp range. For best results, supply ISO 8601 timestamps,
  but `YYYY-mm-dd hh:mm:ss.sss` is also accepted. (GeoHub tries to be flexible
  about this, and may become better over time).
  * `limit`: Return at most this number of entries, starting with the oldest
  entries.
  * `last`: This is a sort of page token, identifying the most recent entry you
  know. GeoHub will only return events newer than this. The IDs used here are
  returned as property `id` in the GeoJSON `Feature`s.
  * Returns a GeoJSON object.
* `GET` `/geo/<client>/retrieve/last?secret=<secret>&last=<last ID>&limit=<max
entries>`
  * Fetch most recent points for the `client`. See `/geo/<client>/retrieve/json`
  above for descriptions of the other parameters.
  * Returns a `LiveUpdate` object. `last` is the most-recent ID of all points:

```json
{
  "type": "GeoHubUpdate",
  "last": 1205,
  "error": "error string if applicable",
  "geo": {
    "type": "FeatureCollection",
    "features": [
      {
        "type": "Feature",
        "properties": {
          "time": "2020-12-03T15:42:40.010325Z",
          "altitude": 40,
          "speed": 22,
          "id": 1205,
          "note": "A happy little note",
        },
        "geometry": {
          "type": "Point",
          "coordinates": [
            6.09,
            50.795
          ]
        }
      },
    ...
    ]
  }
}
```

* `GET` `/geo/<client>/retrieve/live?secret=<secret>&timeout=<timeout in sec>`
  * Wait at most `timeout` seconds for events from `client` with the given
  `secret`. This is a "hanging" request endpoint, returning after `timeout`
  seconds or any time that a new point has been logged. This is useful for
  real-time applications, such as the live map (in `assets/`).
  * This API returns data compatible with `/geo/<client>/retrieve/last`. Think
  of the two endpoints as complementary, one retrieving recent and this one
  returning current events.
  * Note: As opposed to other endpoints, this endpoint doesn't return points
  with lacking secret ("public" points).
  * This endpoint returns at most one point at a time.
  * If no new point has arrived in time, a `LiveUpdate` with `null` entries for
  `geo` and `last` is returned.

## Installation

Installing GeoHub is quite easy. You need

* a PostgreSQL server
* (optional) a reverse proxy in front

1. Set up a database with the supplied `pgsql_schema.sql`. It will install the
   elements into the `geohub` schema. Currently, this is a very small schema.
   Make sure that the `pgcrypto` extension is enabled in your database.
   `PostGIS` is not required.
1. Configure the database connection in `Rocket.toml`. Rocket.rs usually
   connects to PostgreSQL via localhost/::1, so make sure that this is allowed
   by modifying `pg_hba.conf` if needed.
1. If you want TLS or already have a server on port 80/443, configure your main
   webserver to proxy to GeoHub. For example, in nginx you can achieve this very
   easily:

```
# Put this in an existing server { } block.

    # Geohub
    #
    # This will strip the /geo/ prefix, so add it back below. Adapt to your
    # preferred URL scheme.
    location /geo/ {
        proxy_pass http://localhost:8000/geo/;
    }
```

This also allows you to immediately use the `livemap` app at
`https://yourhost.com/geo/assets/livemap.html?client=<yourclient>&secret=verysecret`,
which consists of a single HTML page, a CSS file, and the leaflet.js library
(which is included). - latter is (c) 2010-2019 Vladimir Agafonkin, (c) 2010-2011
CloudMade.

## Usage

![Map data © OpenStreetMap contributors, CC-BY-SA, Imagery © MapBox](screenshot1.png)
*Map data © OpenStreetMap contributors, CC-BY-SA, Imagery © MapBox*

If you want to go on a difficult hike (though one with nice mobile data
coverage) and keep your worried parents up to date, do this:

1. Install the [`GPSLogger` app](https://github.com/mendhak/gpslogger). It is
   the only one I know of that has the kind of feature required for GeoHub.
1. Configure the *Custom URL* feature to your URL. By default, you only
   (optionally) need to add a secret and of course your host and URL part.
1. Start logging, and pass the appropriate link to the livemap
   (`https://yourhost.com/geo/assets/livemap.html?client=<yourclient>&secret=verysecret`)
   to any concerned relatives. If you configure GPSLogger to log every few
   seconds, it will work best. Latency between a point reaching your server and
   the live map being updated will generally be way less than half a second.

See also the `examples/` directory for more ways to use GeoHub. For example,
stream metadata from German long distance trains to GeoHub.

The `assets/` directory contains web applications directly built on top of
GeoHub with nothing more than Ajax and some third-party libraries.
