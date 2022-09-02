use std::time::Duration;

use tokio::sync::mpsc;

pub struct Debouncer<T> {
    tx: mpsc::Sender<T>,
}

impl<T: Eq + Send + Sync + 'static> Debouncer<T> {
    pub fn new<F>(callback: F, time: Duration) -> Self
    where
        F: Fn(T) + Send + 'static,
    {
        let (tx, mut rx) = mpsc::channel(1);

        tokio::spawn(async move {
            let mut last_arg = None;

            loop {
                tokio::select!{
                    current_arg = rx.recv() => {
                        if let Some(current_arg) = current_arg {
                            if last_arg.as_ref() == Some(&current_arg) {
                                // ignore duplicate
                                continue;
                            }
                            // argument changed, send last argument to callback
                            if let Some(last_arg) = last_arg.take() {
                                callback(last_arg);
                            }
                            last_arg = Some(current_arg);
                        } else {
                            // channel closed, send last argument to callback
                            if let Some(last_arg) = last_arg.take() {
                                callback(last_arg);
                            }
                            break;
                        }
                    }
                    _ = tokio::time::sleep(time), if last_arg.is_some() => {
                        // Timeout elapsed, send last argument to callback
                        callback(last_arg.take().unwrap());
                    }
                };
            }
        });

        Self { tx }
    }

    pub async fn call(&self, arg: T) {
        self.tx.send(arg).await.ok();
    }
}
