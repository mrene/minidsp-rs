use std::{
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;
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

impl<T> DerefMut for DropJoinHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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

impl<T> Future for DropJoinHandle<T> {
    type Output = <tokio::task::JoinHandle<T> as Future>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let item = Pin::new(&mut self.get_mut().0);
        item.poll(cx)
    }
}
