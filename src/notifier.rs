use crate::db;
use crate::types;

use fallible_iterator::FallibleIterator;
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time;

/// Request of a web client thread to the notifier thread.
pub struct NotifyRequest {
    pub client: String,
    pub secret: Option<String>,
    pub respond: SendableSender<NotifyResponse>,
}

/// Response from the notifier thread to a web client thread.
pub struct NotifyResponse {
    // The GeoJSON object containing the update and the `last` page token.
    pub geo: Option<types::GeoJSON>,
    pub last: Option<i32>,
}

/// A `Send` sender.
#[derive(Clone)]
pub struct SendableSender<T> {
    pub sender: Arc<Mutex<mpsc::Sender<T>>>,
}

impl<T> SendableSender<T> {
    pub fn send(&self, arg: T) -> Result<(), mpsc::SendError<T>> {
        let s = self.sender.lock().unwrap();
        s.send(arg)
    }
}

fn encode_notify_payload(client: &str, secret: &Option<String>) -> String {
    format!(
        "{} {}",
        client,
        secret.as_ref().map(|s| s.as_str()).unwrap_or("")
    )
}

fn decode_notify_payload(payload: &str) -> (String, Option<String>) {
    let parts = payload.split(' ').collect::<Vec<&str>>();
    assert!(parts.len() >= 1);
    let secret = if parts.len() > 1 {
        Some(parts[1].into())
    } else {
        None
    };
    return (parts[0].into(), secret);
}

/// Build a channel name from a client name and secret.
fn channel_name(client: &str, secret: &str) -> String {
    // The log handler should already have checked this.
    assert!(secret.find(' ').is_none());
    assert!(client.find(' ').is_none());
    format!("geohubclient_update_{}_{}", client, secret)
}

pub struct NotifyManager(pub SendableSender<NotifyRequest>);

impl NotifyManager {
    pub fn wait_for_notification(
        &self,
        client: String,
        secret: Option<String>,
        timeout: Option<u64>,
    ) -> types::LiveUpdate {
        let (send, recv) = mpsc::channel();
        let send = SendableSender {
            sender: Arc::new(Mutex::new(send)),
        };

        let req = NotifyRequest {
            client: client.clone(),
            secret: secret,
            respond: send,
        };
        self.0.send(req).unwrap();

        if let Ok(response) = recv.recv_timeout(time::Duration::new(timeout.unwrap_or(30), 0)) {
            types::LiveUpdate::new(client, response.last, response.geo, None)
        } else {
            types::LiveUpdate::new(client, None, None, Some("timeout, try again".into()))
        }
    }

    pub fn send_notification(
        &self,
        dbq: &db::DBQuery,
        client: &str,
        secret: &Option<String>,
    ) -> Result<u64, postgres::Error> {
        let channel = format!(
            "NOTIFY {}, '{}'",
            channel_name(client, secret.as_ref().unwrap_or(&"".into()).as_str()),
            encode_notify_payload(client, secret),
        );
        let notify = dbq.0.prepare_cached(channel.as_str()).unwrap();
        notify.execute(&[])
    }
}

/// Listen for notifications in the database and dispatch to waiting clients.
pub fn live_notifier_thread(rx: mpsc::Receiver<NotifyRequest>, db: postgres::Connection) {
    const TICK_MILLIS: u32 = 500;

    let mut clients: HashMap<String, Vec<NotifyRequest>> = HashMap::new();
    let db = db::DBQuery(&db);

    fn listen(
        db: &postgres::Connection,
        client: &str,
        secret: &Option<String>,
    ) -> postgres::Result<u64> {
        let n = db
            .execute(
                &format!(
                    "LISTEN {}",
                    channel_name(client, secret.as_ref().map(|s| s.as_str()).unwrap_or(""))
                        .as_str()
                ),
                &[],
            )
            .unwrap();
        Ok(n)
    }
    fn unlisten(
        db: &postgres::Connection,
        client: &str,
        secret: &Option<String>,
    ) -> postgres::Result<u64> {
        let n = db
            .execute(
                &format!(
                    "UNLISTEN {}",
                    channel_name(client, secret.as_ref().map(|s| s.as_str()).unwrap_or(""))
                        .as_str()
                ),
                &[],
            )
            .unwrap();
        Ok(n)
    }

    loop {
        // This loop checks for new messages on rx, then checks for new database notifications, etc.

        // Drain notification requests (clients asking to watch for notifications).
        // We listen per client and secret to separate clients with different sessions (by secret).
        loop {
            if let Ok(nrq) = rx.try_recv() {
                // client_id is also the payload sent to the channel. It keys waiters by client and
                // secret.
                let client_id = encode_notify_payload(nrq.client.as_str(), &nrq.secret);
                if !clients.contains_key(&client_id) {
                    listen(db.0, &nrq.client, &nrq.secret).ok();
                }
                clients.entry(client_id).or_insert(vec![]).push(nrq);
            } else {
                break;
            }
        }

        // Drain notifications from the database.
        // Also provide updated rows to the client.
        let notifications = db.0.notifications();
        let mut iter = notifications.timeout_iter(time::Duration::new(0, TICK_MILLIS * 1_000_000));
        let mut count = 0;

        while let Ok(Some(notification)) = iter.next() {
            // We can extract the client and secret from the channel payload. The payload itself is
            // the hashmap key.
            let client_id = notification.payload;
            let (client, secret) = decode_notify_payload(client_id.as_str());
            unlisten(db.0, client.as_str(), &secret).ok();

            // These queries use the primary key index returning one row only and will be quite fast.
            let rows = db.check_for_new_rows(client.as_str(), &secret, &None, &Some(1));
            if let Some((geo, last)) = rows {
                for request in clients.remove(&client_id).unwrap_or(vec![]) {
                    request
                        .respond
                        .send(NotifyResponse {
                            geo: Some(geo.clone()),
                            last: Some(last),
                        })
                        .ok();
                }
            } else {
                for request in clients.remove(&client_id).unwrap_or(vec![]) {
                    request
                        .respond
                        .send(NotifyResponse {
                            geo: None,
                            last: None,
                        })
                        .ok();
                }
            }

            // We also need to receive new notification requests.
            count += 1;
            if count > 3 {
                break;
            }
        }
    }
}
