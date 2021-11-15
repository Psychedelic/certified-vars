use crate::label::{Label, Prefix};
use crate::{AsHashTree, Hash, HashTree, Map, Seq};
use candid::CandidType;
use serde::{Deserialize, Serialize};
use std::borrow::{Borrow, Cow};

#[derive(CandidType, Serialize, Deserialize, Debug)]
pub struct Paged<K: Label + Ord + 'static, V: AsHashTree + 'static, const S: usize> {
    data: Map<PagedKey<K>, Seq<V>>,
}

#[derive(Ord, CandidType, Serialize, Deserialize, PartialOrd, Eq, PartialEq, Debug)]
struct PagedKey<K: Label + Ord + 'static> {
    key: K,
    page: u32,
}

impl<K: Label + Ord + 'static> Label for PagedKey<K> {
    #[inline]
    fn as_label(&self) -> Cow<[u8]> {
        let mut data = self.key.as_label().to_vec();
        data.extend_from_slice(&self.page.to_be_bytes());
        Cow::Owned(data)
    }
}

impl<K: Label + Ord + 'static> Borrow<K> for PagedKey<K> {
    #[inline]
    fn borrow(&self) -> &K {
        &self.key
    }
}

impl<K: Label + Ord + 'static> Prefix<K> for PagedKey<K> {}

impl<K: Label + Ord + 'static, V: AsHashTree + 'static, const S: usize> Default for Paged<K, V, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Label + Ord + 'static, V: AsHashTree + 'static, const S: usize> Paged<K, V, S> {
    pub fn new() -> Self {
        Self { data: Map::new() }
    }

    pub fn insert(&mut self, key: K, item: V) {
        let tree = &mut self.data.inner;
        let mut item = Some(item);

        let page = tree
            .modify_max_with_prefix(&key, |key, seq| {
                if seq.len() == S {
                    return Some(key.page + 1);
                }
                seq.append(item.take().unwrap());
                None
            })
            .unwrap_or(Some(0));

        if let Some(page) = page {
            let key = PagedKey { key, page };
            let mut value = Seq::new();
            value.append(item.take().unwrap());
            tree.insert(key, value);
        }
    }

    pub fn get_last_page_number(&self, key: &K) -> Option<usize> {
        self.data
            .inner
            .max_entry_with_prefix(key)
            .map(|(k, _)| k.page as usize)
    }

    // TODO(qti3e) Remove the Clone.
    pub fn witness_last_page_number(&self, key: &K) -> HashTree<'_>
    where
        K: Clone,
    {
        let page = self
            .data
            .inner
            .max_entry_with_prefix(key)
            .map(|(k, _)| k.page + 1)
            .unwrap_or(0);
        let key = PagedKey {
            key: key.clone(),
            page,
        };
        self.data.witness(&key)
    }

    // TODO(qti3e) Remove the Clone in future.
    pub fn get(&self, key: &K, page: usize) -> Option<&Seq<V>>
    where
        K: Clone,
    {
        let key = PagedKey {
            key: key.clone(),
            page: page as u32,
        };
        self.data.get(&key)
    }

    // TODO(qti3e) Remove the Clone in future.
    pub fn witness(&self, key: &K, page: usize) -> HashTree<'_>
    where
        K: Clone,
    {
        let key = PagedKey {
            key: key.clone(),
            page: page as u32,
        };
        self.data.witness(&key)
    }
}

impl<K: Label + Ord + 'static, V: AsHashTree + 'static, const S: usize> AsHashTree
    for Paged<K, V, S>
{
    fn root_hash(&self) -> Hash {
        self.data.root_hash()
    }

    fn as_hash_tree(&self) -> HashTree<'_> {
        self.data.as_hash_tree()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modify_max_with_prefix() {
        let mut paged = Paged::<i32, i32, 3>::new();
        paged.data.append_deep(PagedKey { key: 1, page: 0 }, 0);
        paged.data.append_deep(PagedKey { key: 1, page: 0 }, 1);
        paged.data.append_deep(PagedKey { key: 1, page: 0 }, 2);
        paged.data.append_deep(PagedKey { key: 1, page: 1 }, 3);
        paged.data.append_deep(PagedKey { key: 1, page: 1 }, 4);
        paged.data.append_deep(PagedKey { key: 1, page: 1 }, 5);
        paged.data.append_deep(PagedKey { key: 1, page: 2 }, 18);

        paged.data.append_deep(PagedKey { key: 3, page: 0 }, 6);
        paged.data.append_deep(PagedKey { key: 3, page: 0 }, 7);
        paged.data.append_deep(PagedKey { key: 3, page: 0 }, 8);
        paged.data.append_deep(PagedKey { key: 3, page: 1 }, 9);
        paged.data.append_deep(PagedKey { key: 3, page: 1 }, 10);
        paged.data.append_deep(PagedKey { key: 3, page: 1 }, 11);

        paged.data.append_deep(PagedKey { key: 5, page: 0 }, 12);
        paged.data.append_deep(PagedKey { key: 5, page: 0 }, 13);
        paged.data.append_deep(PagedKey { key: 5, page: 0 }, 14);
        paged.data.append_deep(PagedKey { key: 5, page: 1 }, 15);
        paged.data.append_deep(PagedKey { key: 5, page: 1 }, 16);
        paged.data.append_deep(PagedKey { key: 5, page: 1 }, 17);

        assert_eq!(paged.data.inner.modify_max_with_prefix(&0, |k, _| k), None);

        assert_eq!(
            paged.data.inner.modify_max_with_prefix(&1, |k, _| k),
            Some(&PagedKey { key: 1, page: 2 })
        );

        assert_eq!(paged.data.inner.modify_max_with_prefix(&2, |k, _| k), None);

        assert_eq!(
            paged.data.inner.modify_max_with_prefix(&3, |k, _| k),
            Some(&PagedKey { key: 3, page: 1 })
        );

        assert_eq!(paged.data.inner.modify_max_with_prefix(&4, |k, _| k), None);

        assert_eq!(
            paged.data.inner.modify_max_with_prefix(&5, |k, _| k),
            Some(&PagedKey { key: 5, page: 1 })
        );

        assert_eq!(paged.data.inner.modify_max_with_prefix(&6, |k, _| k), None);
    }
}
