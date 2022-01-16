use std::collections::HashMap;
//use std::sync::mpsc::{channel, Receiver, Sender};
use async_channel::{unbounded, Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};
use tracing::*;

lazy_static::lazy_static! {
    static ref BROKER: Arc<Mutex<Broker>> = Broker::new();
}

/// This is the type which is sent from the publisher to subscribers
type TransportType = Vec<u8>;

/// A publisher allows sending replay fragments to the broker, which parses
/// them and forwards the packets to mailboxes.
pub struct Publisher {
    history: Vec<TransportType>,
    senders: Vec<Weak<Mutex<Sender<TransportType>>>>,
    broker: BrokerProxy,
}

impl Publisher {
    pub async fn upload(&mut self, data: TransportType) {
        for sub in self.senders.iter_mut() {
            if let Some(sub) = sub.upgrade() {
                let sub = sub.lock().unwrap();
                sub.send(data.clone()).await.unwrap();
            }
        }
        self.history.push(data);
    }

    /*pub fn set_username(&mut self, username: &str) {
        let mut b = self.broker.0.lock().unwrap();
        self.senders = b.clone_subscribers(username);
    }*/

    fn add_subscriber(&mut self, subscriber: Weak<Mutex<Sender<TransportType>>>) {
        if let Some(subscriber) = subscriber.upgrade() {
            // Update them with the history
            let subscriber = subscriber.lock().unwrap();
            for item in self.history.iter() {
                // If the mailbox has already hung up, that's fine
                let _ = subscriber.send(item.clone());
            }
        }
        self.senders.push(subscriber);
    }
}

pub struct PublisherProxy(Arc<Mutex<Publisher>>);

impl PublisherProxy {
    pub async fn upload(&mut self, data: TransportType) {
        let mut p = self.0.lock().unwrap();
        p.upload(data).await;
    }

    pub fn set_username(&mut self, username: &str) {
        let mut p = self.0.lock().unwrap();
        //p.set_username(username);
        let senders = {
            let mut b = p.broker.0.lock().unwrap();
            let senders = b.clone_subscribers(username);
            b.register_publisher(username, Arc::downgrade(&self.0));
            senders
        };
        p.senders = senders;
    }
}

/// An object which allows subscribers to retrieve in real-time the packets.
pub struct Mailbox {
    /// Hold a strong reference to the sender so that all of the weak references
    /// drop when this mailbox is dropped
    sender: Arc<Mutex<Sender<TransportType>>>,
    receiver: Receiver<TransportType>,
}

impl Mailbox {
    /*pub fn try_recv(&mut self) -> Result<TransportType, std::sync::mpsc::TryRecvError> {
        self.receiver.try_recv()
    }*/
    pub async fn recv(&mut self) -> Result<TransportType, async_channel::RecvError> {
        self.receiver.recv().await
    }
}

struct Broker {
    //publishers: HashMap<String, Publisher>,
    publishers: HashMap<String, Weak<Mutex<Publisher>>>,
    subscribers: HashMap<String, Vec<Weak<Mutex<Sender<TransportType>>>>>,
}

impl Broker {
    /// Can only be constructed in the static
    fn new() -> Arc<Mutex<Broker>> {
        Arc::new(Mutex::new(Broker {
            publishers: HashMap::new(),
            subscribers: HashMap::new(),
        }))
    }

    fn register_subscriber(
        &mut self,
        username: &str,
        subscriber: Weak<Mutex<Sender<TransportType>>>,
    ) {
        if !self.subscribers.contains_key(username) {
            self.subscribers.insert(username.to_owned(), vec![]);
        }
        self.subscribers
            .get_mut(username)
            .unwrap()
            .push(subscriber.clone());

        // Check if this publisher already is registered
        if let Some(publisher) = self.publishers.get(username) {
            if let Some(publisher) = publisher.upgrade() {
                let mut publisher = publisher.lock().unwrap();
                publisher.add_subscriber(subscriber);
            }
        }
    }

    fn register_publisher(&mut self, username: &str, publisher: Weak<Mutex<Publisher>>) {
        if self.publishers.contains_key(username) {
            // If the current publisher is still active, that's a problem. Only one publisher
            // may be active for a given username (how would a single username be playing
            // multiple games?)
            if let Some(_) = self.publishers.get(username).unwrap().upgrade() {
                // TODO: Figure out how to handle this (do we drop this one, or replace the old one?)
                panic!("Multiple subscribers for username {}!", username);
            } else {
                // The old publisher is done, clear the entry
                info!("Previous publisher for username {} is dropped", username);
                self.publishers.remove(username);
            }
        }
        self.publishers.insert(username.to_owned(), publisher);
    }

    fn clone_subscribers(&self, username: &str) -> Vec<Weak<Mutex<Sender<TransportType>>>> {
        self.subscribers.get(username).unwrap_or(&vec![]).clone()
    }
}

struct BrokerProxy(Arc<Mutex<Broker>>);

impl BrokerProxy {
    pub fn get() -> Self {
        Self(BROKER.clone())
    }

    pub fn subscribe(&mut self, username: &str) -> Mailbox {
        let (sender, receiver) = unbounded();
        let sender = Arc::new(Mutex::new(sender));

        // Register the subscriber
        {
            let mut b = self.0.lock().unwrap();
            b.register_subscriber(username, Arc::downgrade(&sender));
        }

        Mailbox { sender, receiver }
    }

    pub fn publish(&mut self) -> PublisherProxy {
        // We don't know the username of this upload yet
        PublisherProxy(Arc::new(Mutex::new(Publisher {
            history: vec![],
            senders: vec![],
            broker: BrokerProxy(self.0.clone()),
        })))
    }
}

// These tests only work when TransportType == Vec<u8>
/*#[cfg(test)]
mod test {
    use super::*;
    use std::sync::mpsc::TryRecvError;

    #[test]
    fn test_single_sub_then_pub() {
        let mut b = Broker::new();
        let mut b = BrokerProxy(b);

        let mut mailbox = b.subscribe("a");
        let mut publisher = b.publish();
        publisher.set_username("a");

        assert_eq!(mailbox.try_recv(), Err(TryRecvError::Empty));

        publisher.upload(vec![1, 2, 3]);

        assert_eq!(mailbox.try_recv(), Ok(vec![1, 2, 3]));
    }

    #[test]
    fn test_single_pub_then_sub() {
        let mut b = Broker::new();
        let mut b = BrokerProxy(b);

        let mut publisher = b.publish();
        publisher.set_username("a");

        let mut mailbox = b.subscribe("a");

        assert_eq!(mailbox.try_recv(), Err(TryRecvError::Empty));

        publisher.upload(vec![1, 2, 3]);

        assert_eq!(mailbox.try_recv(), Ok(vec![1, 2, 3]));
    }

    #[test]
    fn test_delayed_subscription_backfill() {
        let mut b = Broker::new();
        let mut b = BrokerProxy(b);

        let mut publisher = b.publish();
        publisher.set_username("a");

        publisher.upload(vec![1, 2, 3]);

        let mut mailbox = b.subscribe("a");
        assert_eq!(mailbox.try_recv(), Ok(vec![1, 2, 3]));
    }

    #[test]
    fn test_delayed_subscription_backfill_multi() {
        let mut b = Broker::new();
        let mut b = BrokerProxy(b);

        let mut publisher = b.publish();
        publisher.set_username("a");

        publisher.upload(vec![1, 2, 3]);
        publisher.upload(vec![4, 5, 6]);

        let mut mailbox = b.subscribe("a");
        assert_eq!(mailbox.try_recv(), Ok(vec![1, 2, 3]));
        assert_eq!(mailbox.try_recv(), Ok(vec![4, 5, 6]));
    }

    #[test]
    fn test_re_publishing_while_subscribed() {
        let mut b = Broker::new();
        let mut b = BrokerProxy(b);

        let mut publisher = b.publish();
        publisher.set_username("a");

        publisher.upload(vec![1, 2, 3]);
        publisher.upload(vec![4, 5, 6]);

        let mut mailbox = b.subscribe("a");
        assert_eq!(mailbox.try_recv(), Ok(vec![1, 2, 3]));
        assert_eq!(mailbox.try_recv(), Ok(vec![4, 5, 6]));

        std::mem::drop(publisher);

        assert_eq!(mailbox.try_recv(), Err(TryRecvError::Empty));

        let mut publisher = b.publish();
        publisher.set_username("a");

        publisher.upload(vec![7, 8, 9]);

        assert_eq!(mailbox.try_recv(), Ok(vec![7, 8, 9]));
    }
}*/
