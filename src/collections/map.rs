use crate::collections::seq::Seq;
use crate::rbtree::RbTree;
use crate::AsHashTree;
use std::collections::HashMap;

#[derive(Default)]
pub struct Map<K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> {
    inner: RbTree<K, V>,
}

impl<K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> Map<K, V> {
    /// Returns `true` if the map does not contain any values.
    #[inline]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Insert a key-value pair into the map. Returns [`None`] if the key did not
    /// exists in the map, otherwise the previous value associated with the provided
    /// key will be returned.
    #[inline]
    fn insert(&mut self, key: K, value: V) -> Option<V> {
        // self.inner.insert(key, value)
        todo!()
    }
}

impl<K: 'static + AsRef<[u8]>, V: AsHashTree> Map<K, Seq<V>> {}
