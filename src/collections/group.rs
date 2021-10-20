use crate::hashtree::HashTree::Pruned;
use crate::hashtree::{fork_hash, labeled_hash, ForkInner};
use crate::{AsHashTree, Hash, HashTree};
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

pub mod builder;

type NodeId = u64;

/// Group is a utility structure to make it easier to deal with multiple nested
/// certified data in one canister.
pub struct Group {
    /// The root node of the group is a shadow of the shape of the group's tree.
    root: GroupNode,
    /// The data in this group.
    data: HashMap<TypeId, Box<dyn GroupLeaf>>,
    /// Map each typeId used in a Leaf node to all of its ancestors.
    dependencies: HashMap<TypeId, Vec<NodeId>>,
}

pub struct Ray<'a> {
    /// The group this ray belongs to.
    group: &'a Group,
    /// The union of all the ancestors of nodes that we're interested in.
    to_visit: HashSet<NodeId>,
    /// The [`HashTree`] that should be used for each leaf that we're interested
    /// in.
    leaves: HashMap<TypeId, HashTree<'a>>,
}

#[derive(Debug)]
struct GroupNode {
    id: NodeId,
    data: GroupNodeInner,
}

#[derive(Debug)]
enum GroupNodeInner {
    Fork(Box<GroupNode>, Box<GroupNode>),
    Labeled(String, Box<GroupNode>),
    Leaf(TypeId),
}

impl Group {
    /// Visit all the nodes recursively and assign the ID and extract the dependencies.
    fn init(&mut self) {
        self.dependencies.clear();
        let mut path = Vec::with_capacity(16);
        self.root.visit_node(0, &mut self.dependencies, &mut path);
    }

    /// Create a new witness builder that can be used to generate a [`HashTree`] for
    /// the entire group.
    #[must_use = "This method does not have any effects on the group."]
    pub fn witness(&self) -> Ray {
        Ray::new(self)
    }

    /// Returns a mutable reference to the leaf node with the given type.
    ///
    /// # Panics
    ///
    /// This method panics if the group does not contain any leaf nodes with the given
    /// type.
    pub fn get_mut<T: GroupLeaf>(&mut self) -> &mut T {
        let tid = TypeId::of::<T>();
        self.data
            .get_mut(&tid)
            .expect("Group does not contain the type")
            .downcast_mut()
            .unwrap()
    }

    /// Returns a reference to the leaf node with the given type.
    ///
    /// # Panics
    ///
    /// This method panics if the group does not contain any leaf nodes with the given
    /// type.
    pub fn get<T: GroupLeaf>(&self) -> &T {
        let tid = TypeId::of::<T>();
        self.data
            .get(&tid)
            .expect("Group does not contain the type")
            .downcast_ref()
            .unwrap()
    }
}

impl GroupNode {
    /// Assign the ID of this node, this will recursively update the ID of all the child nodes.
    #[inline]
    fn visit_node(
        &mut self,
        id: NodeId,
        dependencies: &mut HashMap<TypeId, Vec<NodeId>>,
        path: &mut Vec<NodeId>,
    ) -> NodeId {
        match &mut self.data {
            GroupNodeInner::Fork(left, right) => {
                self.id = id;
                path.push(self.id);
                let next_id = left.visit_node(id + 1, dependencies, path);
                let next_id = right.visit_node(next_id, dependencies, path);
                path.pop();
                next_id
            }
            GroupNodeInner::Leaf(tid) => {
                path.push(id);
                dependencies.insert(*tid, path.clone());
                path.pop();
                self.id = id;
                id + 1
            }
            GroupNodeInner::Labeled(_, node) => {
                path.push(id);
                let next_id = node.visit_node(id + 1, dependencies, path);
                path.pop();
                self.id = id;
                next_id
            }
        }
    }

    fn witness<'r>(&'r self, ray: &mut Ray<'r>) -> HashTree<'r> {
        if !ray.to_visit.contains(&self.id) {
            return Pruned(self.root_hash(ray.group));
        }

        match &self.data {
            GroupNodeInner::Fork(left, right) => {
                let l_tree = left.witness(ray);
                let r_tree = right.witness(ray);
                HashTree::Fork(Box::new(ForkInner(l_tree, r_tree)))
            }
            GroupNodeInner::Labeled(label, n) => {
                let tree = n.witness(ray);
                HashTree::Labeled(Cow::Borrowed(label.as_bytes()), Box::new(tree))
            }
            GroupNodeInner::Leaf(tid) => ray.leaves.remove(tid).unwrap(),
        }
    }

    fn witness_all<'a>(&'a self, group: &'a Group) -> HashTree<'a> {
        match &self.data {
            GroupNodeInner::Fork(left, right) => {
                let l_tree = left.witness_all(group);
                let r_tree = right.witness_all(group);
                HashTree::Fork(Box::new(ForkInner(l_tree, r_tree)))
            }
            GroupNodeInner::Labeled(label, n) => {
                let tree = n.witness_all(group);
                HashTree::Labeled(Cow::Borrowed(label.as_bytes()), Box::new(tree))
            }
            GroupNodeInner::Leaf(tid) => group.data.get(tid).unwrap().as_hash_tree(),
        }
    }

    fn root_hash(&self, group: &Group) -> Hash {
        match &self.data {
            GroupNodeInner::Fork(left, right) => {
                fork_hash(&left.root_hash(group), &right.root_hash(group))
            }
            GroupNodeInner::Labeled(label, node) => {
                let hash = node.root_hash(group);
                labeled_hash(label.as_bytes(), &hash)
            }
            GroupNodeInner::Leaf(id) => group.data.get(id).unwrap().root_hash(),
        }
    }
}

impl<'a> Ray<'a> {
    fn new(group: &'a Group) -> Self {
        Self {
            group,
            to_visit: HashSet::with_capacity(16),
            leaves: HashMap::with_capacity(8),
        }
    }

    #[must_use = "Computing a HashTree is a compute heavy operation, with zero effects on the Group."]
    pub fn build(mut self) -> HashTree<'a> {
        self.group.root.witness(&mut self)
    }

    #[must_use]
    pub fn full<T: GroupLeaf + 'static>(mut self) -> Self {
        let tid = TypeId::of::<T>();

        for dep in self.group.dependencies.get(&tid).unwrap() {
            self.to_visit.insert(*dep);
        }

        let tree = self.group.data.get(&tid).unwrap().as_hash_tree();
        self.leaves.insert(tid, tree);

        self
    }

    #[must_use]
    pub fn partial<T: GroupLeaf + 'static, F: FnOnce(&T) -> HashTree>(mut self, f: F) -> Self {
        let tid = TypeId::of::<T>();

        for dep in self.group.dependencies.get(&tid).unwrap() {
            self.to_visit.insert(*dep);
        }

        let data = self.group.data.get(&tid).unwrap();
        let tree = f(data.downcast_ref().unwrap());
        self.leaves.insert(tid, tree);

        self
    }
}

pub trait GroupLeaf: Any + AsHashTree {}
impl<T: Any + AsHashTree> GroupLeaf for T {}

impl dyn GroupLeaf {
    pub fn is<T: GroupLeaf>(&self) -> bool {
        let t = TypeId::of::<T>();
        let concrete = self.type_id();
        t == concrete
    }

    pub fn downcast_ref<T: GroupLeaf>(&self) -> Option<&T> {
        if self.is::<T>() {
            unsafe { Some(&*(self as *const dyn GroupLeaf as *const T)) }
        } else {
            None
        }
    }

    pub fn downcast_mut<T: GroupLeaf>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            unsafe { Some(&mut *(self as *mut dyn GroupLeaf as *mut T)) }
        } else {
            None
        }
    }
}

impl AsHashTree for Group {
    fn root_hash(&self) -> Hash {
        self.root.root_hash(self)
    }

    fn as_hash_tree(&self) -> HashTree<'_> {
        self.root.witness_all(self)
    }
}

#[test]
fn xxx() {
    use crate::Map;
    let mut map = Map::<String, i8>::new();
    map.insert("X".to_string(), 17);

    println!("Hash : {}", hex::encode(map.root_hash()));

    let data: Box<dyn GroupLeaf> = Box::new(map);

    let as_map = data.downcast_ref::<Map<String, i8>>();
    println!("As map: {:?}", as_map);
}

#[test]
fn yyy() {
    use crate::Map;
    type StringToI8Map = Map<String, i8>;
    let mut map = StringToI8Map::new();
    map.insert("X".to_string(), 17);

    let mut group = Group {
        root: GroupNode {
            id: 0,
            data: GroupNodeInner::Fork(
                Box::new(GroupNode {
                    id: 0,
                    data: GroupNodeInner::Labeled(
                        "A".into(),
                        Box::new(GroupNode {
                            id: 0,
                            data: GroupNodeInner::Leaf(TypeId::of::<StringToI8Map>()),
                        }),
                    ),
                }),
                Box::new(GroupNode {
                    id: 0,
                    data: GroupNodeInner::Leaf(TypeId::of::<i8>()),
                }),
            ),
        },
        data: Default::default(),
        dependencies: Default::default(),
    };

    group.data.insert(TypeId::of::<i8>(), Box::new(17));
    group
        .data
        .insert(TypeId::of::<StringToI8Map>(), Box::new(map));
    group.init();

    let t1 = group.witness().build();
    let t2 = group.witness().full::<i8>().build();
    let t3 = group.witness().full::<StringToI8Map>().build();
    let t4 = group
        .witness()
        .partial(|map: &StringToI8Map| map.witness(&"X".into()))
        .build();

    assert_eq!(t1.reconstruct(), t2.reconstruct());
    assert_eq!(t1.reconstruct(), t3.reconstruct());
    assert_eq!(t1.reconstruct(), t4.reconstruct());

    println!("{:#?}", t4);
}
