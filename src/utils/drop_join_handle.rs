use std::ops::Deref;

use tokio::task::JoinHandle;
/// Cancels a tokio task when dropped
pub struct DropJoinHandle<T>(JoinHandle<T>);

impl<T> DropJoinHandle<T> {
    pub fn new(inner: JoinHandle<T>) -> Self {
        Self(inner)
    }
}

impl<T> Deref for DropJoinHandle<T> {
    type Target = JoinHandle<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Drop for DropJoinHandle<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl<T> From<JoinHandle<T>> for DropJoinHandle<T> {
    fn from(from: JoinHandle<T>) -> Self {
        Self::new(from)
    }
}
