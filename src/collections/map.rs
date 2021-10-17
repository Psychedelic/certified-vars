use crate::collections::seq::Seq;
use crate::rbtree::entry::Entry;
use crate::rbtree::iterator::RbTreeIterator;
use crate::rbtree::RbTree;
use crate::AsHashTree;
use candid::types::{Compound, Field, Label, Type};
use candid::CandidType;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug, Formatter};
use std::iter::FromIterator;
use std::marker::PhantomData;

#[derive(Default)]
pub struct Map<K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> {
    inner: RbTree<K, V>,
}

impl<K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> Map<K, V> {
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: RbTree::new(),
        }
    }

    /// Returns `true` if the map does not contain any values.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Clear the map.
    #[inline]
    pub fn clear(&mut self) {
        self.inner = RbTree::new();
    }

    /// Insert a key-value pair into the map. Returns [`None`] if the key did not
    /// exists in the map, otherwise the previous value associated with the provided
    /// key will be returned.
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value).0
    }

    /// Remove the value associated with the given key from the map, returns the
    /// previous value associated with the key.
    #[inline]
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.delete(key.as_ref()).map(|(_, v)| v)
    }

    /// Remove an entry from the map and return the key and value.
    #[inline]
    pub fn remove_entry(&mut self, key: &K) -> Option<(K, V)> {
        self.inner.delete(key.as_ref())
    }

    #[inline]
    pub fn entry(&mut self, key: K) -> Entry<K, V> {
        self.inner.entry(key)
    }

    /// Return the value associated with the given key.
    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key.as_ref())
    }

    /// Return an iterator over the key-values in the map.
    #[inline]
    pub fn iter(&self) -> RbTreeIterator<K, V> {
        RbTreeIterator::new(&self.inner)
    }
}

impl<K: 'static + AsRef<[u8]>, V: AsHashTree> Map<K, Seq<V>> {
    /// Perform a [`Seq::append`] on the seq associated with the give value, if
    /// the seq does not exists, creates an empty one and inserts it to the map.
    pub fn append_deep(&mut self, key: K, value: V) {
        let mut value = Some(value);

        self.inner.modify(key.as_ref(), |seq| {
            seq.append(value.take().unwrap());
        });

        if let Some(value) = value.take() {
            let mut seq = Seq::new();
            seq.append(value);
            self.inner.insert(key, seq);
        }
    }

    #[inline]
    pub fn len_deep(&mut self, key: &K) -> usize {
        self.inner
            .get(key.as_ref())
            .map(|seq| seq.len())
            .unwrap_or(0)
    }
}

impl<K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> Serialize for Map<K, V>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_map(Some(self.len()))?;

        for (key, value) in self.iter() {
            s.serialize_entry(key, value)?;
        }

        s.end()
    }
}

impl<'de, K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> Deserialize<'de> for Map<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(MapVisitor(PhantomData::default()))
    }
}

struct MapVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> Visitor<'de> for MapVisitor<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = Map<K, V>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "expected a map")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut result = Map::new();

        loop {
            if let Some((key, value)) = map.next_entry::<K, V>()? {
                result.insert(key, value);
                continue;
            }

            break;
        }

        Ok(result)
    }
}

impl<K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> CandidType for Map<K, V>
where
    K: CandidType,
    V: CandidType,
{
    fn _ty() -> Type {
        let tuple = Type::Record(vec![
            Field {
                id: Label::Id(0),
                ty: K::ty(),
            },
            Field {
                id: Label::Id(1),
                ty: V::ty(),
            },
        ]);
        Type::Vec(Box::new(tuple))
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        let mut ser = serializer.serialize_vec(self.len())?;
        for e in self.iter() {
            Compound::serialize_element(&mut ser, &e)?;
        }
        Ok(())
    }
}

impl<K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> FromIterator<(K, V)> for Map<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut result = Map::new();

        for (key, value) in iter {
            result.insert(key, value);
        }

        result
    }
}

impl<K: 'static + AsRef<[u8]>, V: AsHashTree + 'static> Debug for Map<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert() {
        let mut map = Map::<String, u32>::new();
        assert_eq!(map.insert("A".into(), 0), None);
        assert_eq!(map.insert("A".into(), 1), Some(0));
        assert_eq!(map.insert("B".into(), 2), None);
        assert_eq!(map.insert("C".into(), 3), None);
        assert_eq!(map.insert("B".into(), 4), Some(2));
        assert_eq!(map.insert("C".into(), 5), Some(3));
        assert_eq!(map.insert("B".into(), 6), Some(4));
        assert_eq!(map.insert("C".into(), 7), Some(5));
        assert_eq!(map.insert("A".into(), 8), Some(1));

        assert_eq!(map.get(&"A".into()), Some(&8));
        assert_eq!(map.get(&"B".into()), Some(&6));
        assert_eq!(map.get(&"C".into()), Some(&7));
        assert_eq!(map.get(&"D".into()), None);
    }

    #[test]
    fn remove() {
        let mut map = Map::<String, u32>::new();

        for i in 0..200u32 {
            map.insert(hex::encode(&i.to_be_bytes()), i);
        }

        for i in 0..200u32 {
            assert_eq!(map.remove(&hex::encode(&i.to_be_bytes())), Some(i));
        }

        for i in 0..200u32 {
            assert_eq!(map.get(&hex::encode(&i.to_be_bytes())), None);
        }
    }

    #[test]
    fn remove_rev() {
        let mut map = Map::<String, u32>::new();

        for i in 0..200u32 {
            map.insert(hex::encode(&i.to_be_bytes()), i);
        }

        for i in (0..200u32).rev() {
            assert_eq!(map.remove(&hex::encode(&i.to_be_bytes())), Some(i));
        }

        for i in 0..200u32 {
            assert_eq!(map.get(&hex::encode(&i.to_be_bytes())), None);
        }
    }
}
