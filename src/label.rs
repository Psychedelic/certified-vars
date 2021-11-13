use std::borrow::{Borrow, Cow};

/// Any value that can be used as a label in the [`HashTree`] and can be a key
/// in the [`RbTree`].
///
/// [`HashTree`]: crate::HashTree
/// [`RbTree`]: crate::rbtree::RbTree
pub trait Label: Ord {
    fn as_label(&self) -> Cow<[u8]>;
}

/// A type `T` can be defined as prefix of type `U`, if they follow the same
/// representation and any valid value of `T` is also a valid head for a value
/// of type `U`.
///
/// The implementation should guarantee that the ordering is preserved which
/// implies:  
/// For any `u: U = x . y` where `x: T`:  
/// 1. `x0 < x1 => u0 < u1`  
/// 2. `x0 > x1 => u0 > u1`  
/// 3. `u0 == u1 => x0 == x1`
///
/// To implement this type, the Self (i.e `U`) should be borrowable as a `T`.
pub trait Prefix<T: Ord + ?Sized>: Label + Borrow<T> {
    /// Check if the provided value is the prefix of self. The default
    /// implementation only extracts the prefix from Self and checks
    /// for their equality which might not be true for some cases
    /// where we're dealing with slices of variable length for example.
    fn is_prefix(&self, prefix: &T) -> bool {
        self.borrow() == prefix
    }
}

impl<T: Ord + AsRef<[u8]>> Label for T {
    fn as_label(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_ref())
    }
}
