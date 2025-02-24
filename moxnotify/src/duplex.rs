use tokio::sync::broadcast;

struct Duplex<T> {
    sender: broadcast::Sender<T>,
    receiver: broadcast::Receiver<T>,
    reverse_sender: broadcast::Sender<T>,
    reverse_receiver: broadcast::Receiver<T>,
}

impl<T: Clone> Duplex<T> {
    fn channel() -> (Self, Self) {
        let (tx1, rx1) = broadcast::channel(16);
        let (tx2, rx2) = broadcast::channel(16);

        let duplex1 = Self {
            sender: tx1.clone(),
            receiver: rx2,
            reverse_sender: tx2.clone(),
            reverse_receiver: rx1,
        };

        let duplex2 = Self {
            sender: tx2,
            receiver: rx1,
            reverse_sender: tx1,
            reverse_receiver: rx2,
        };

        (duplex1, duplex2)
    }
}
