use std::borrow::Cow;

/// Any value that can be used as a label in the [`HashTree`] and can be a key
/// in the [`RbTree`].
///
/// [`HashTree`]: crate::HashTree
/// [`RbTree`]: crate::rbtree::RbTree
pub trait Label: Ord {
    fn as_label(&self) -> Cow<[u8]>;

    fn is_prefix_of(&self, other: &Self) -> bool;
}

impl<T: Ord + AsRef<[u8]>> Label for T {
    fn as_label(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_ref())
    }

    fn is_prefix_of(&self, other: &Self) -> bool {
        let p = self.as_ref();
        let x = other.as_ref();
        if p.len() > x.len() {
            return false;
        }
        &x[0..p.len()] == p
    }
}
