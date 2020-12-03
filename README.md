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

* `/geo/<client>/log?lat=<latitude>&longitude=<longitude>&time=<time>&s=<speed>&ele=<elevation>&secret=<secret>`
  * Log a new point.
  * `latitude`, `longitude`: Geographical position, in decimal degrees (note:
      may be extended later). **Required**.
  * `time`: ISO 8601 time. If left out, current server time is used. Example:
  `2020-12-03T15:42:40.010325Z`. **Optional**.
  * `speed`: Speed in km/h (usually). If you decide to always use m/s, you are
  free to do so. **Optional**.
  * `elevation`: Elevation in meters. **Optional**.
  * Usually returns code **200** except for server errors (500) or malformed inputs (400).
* `/geo/assets/...`
  * Static file serving. The `assets` directory should be deployed in the
  current working directory from which the server is run.
* `/geo/<client>/retrieve/json?secret=<secret>&from=<from_timestamp>&to=<to_timestamp>&limit=<maximum
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
* `/geo/<client>/retrieve/last?secret=<secret>&last=<last ID>&limit=<max
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
          "id": 1205
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

* `/geo/<client>/retrieve/live?secret=<secret>&timeout=<timeout in sec>`
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
