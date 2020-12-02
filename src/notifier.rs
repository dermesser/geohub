
use crate::db;
use crate::ids;
use crate::types;

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time;
use fallible_iterator::FallibleIterator;

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

/// Listen for notifications in the database and dispatch to waiting clients.
pub fn live_notifier_thread(rx: mpsc::Receiver<NotifyRequest>, db: postgres::Connection) {
    const TICK_MILLIS: u32 = 500;

    let mut clients: HashMap<String, Vec<NotifyRequest>> = HashMap::new();

    fn listen(db: &postgres::Connection, client: &str, secret: &str) -> postgres::Result<u64> {
        let n = db
            .execute(
                &format!("LISTEN {}", ids::channel_name(client, secret).as_str()),
                &[],
            )
            .unwrap();
        Ok(n)
    }
    fn unlisten(db: &postgres::Connection, chan: &str) -> postgres::Result<u64> {
        let n = db.execute(&format!("UNLISTEN {}", chan), &[]).unwrap();
        Ok(n)
    }

    loop {
        // This loop checks for new messages on rx, then checks for new database notifications, etc.

        // Drain notification requests (clients asking to watch for notifications).
        // We listen per client and secret to separate clients with different sessions (by secret).
        loop {
            if let Ok(nrq) = rx.try_recv() {
                let secret = nrq.secret.as_ref().map(|s| s.as_str()).unwrap_or("");
                let chan_name = ids::channel_name(nrq.client.as_str(), secret);
                if !clients.contains_key(&chan_name) {
                    listen(&db, &nrq.client, secret).ok();
                }
                clients.entry(chan_name).or_insert(vec![]).push(nrq);
            } else {
                break;
            }
        }

        // Drain notifications from the database.
        // Also provide updated rows to the client.
        let notifications = db.notifications();
        let mut iter = notifications.timeout_iter(time::Duration::new(0, TICK_MILLIS * 1_000_000));
        let mut count = 0;

        while let Ok(Some(notification)) = iter.next() {
            let chan = notification.channel;
            let (client, secret) = ids::client_secret(chan.as_str());
            unlisten(&db, &chan).ok();

            // These queries use the primary key index returning one row only and will be quite fast.
            // Still: One query per client.
            let rows = db::check_for_new_rows(&db, client, Some(secret), &None, &Some(1));
            if let Some((geo, last)) = rows {
                for request in clients.remove(&chan).unwrap_or(vec![]) {
                    request
                        .respond
                        .send(NotifyResponse {
                            geo: Some(geo.clone()),
                            last: Some(last),
                        })
                        .ok();
                }
            } else {
                for request in clients.remove(&chan).unwrap_or(vec![]) {
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

