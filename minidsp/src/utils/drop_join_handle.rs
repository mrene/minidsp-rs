use std::{
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;
use tokio::task::JoinHandle;
/// Cancels a tokio task when dropped
pub struct OwnedJoinHandle<T>(JoinHandle<T>);

impl<T> OwnedJoinHandle<T> {
    pub fn new(inner: JoinHandle<T>) -> Self {
        Self(inner)
    }
}

impl<T> Deref for OwnedJoinHandle<T> {
    type Target = JoinHandle<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for OwnedJoinHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Drop for OwnedJoinHandle<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl<T> From<JoinHandle<T>> for OwnedJoinHandle<T> {
    fn from(from: JoinHandle<T>) -> Self {
        Self::new(from)
    }
}

impl<T> Future for OwnedJoinHandle<T> {
    type Output = <tokio::task::JoinHandle<T> as Future>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let item = Pin::new(&mut self.get_mut().0);
        item.poll(cx)
    }
}
