use crate::hashtree::HashTree::Pruned;
use crate::hashtree::{fork_hash, labeled_hash, leaf_hash, ForkInner};
use crate::{AsHashTree, Hash, HashTree, Map};
use std::any::{Any, TypeId};
use std::cmp::max;
use std::collections::{HashMap, HashSet};

type NodeId = u64;

/// Group is a utility structure to make it easier to deal with multiple certified data
/// in one canister.
pub struct Group {
    root: GroupNode,
    data: HashMap<TypeId, Box<dyn GroupLeaf>>,
    /// Map each typeId used in a Leaf node to all of its ancestors.
    dependencies: HashMap<TypeId, Vec<NodeId>>,
}

pub struct Ray<'a> {
    group: &'a Group,
    /// The union of all the ancestors of nodes that we're interested in.
    to_visit: HashSet<NodeId>,
    interests: HashMap<TypeId, HashTree<'a>>,
}

struct GroupNode {
    id: NodeId,
    data: GroupNodeInner,
}

enum GroupNodeInner {
    Empty,
    Fork(Box<GroupNode>, Box<GroupNode>),
    Labeled(String, Box<GroupNode>),
    Leaf(TypeId),
}

impl Group {
    fn init(&mut self) {
        let mut path = Vec::with_capacity(16);
        self.root
            .assign_id_recursive(0, &mut self.dependencies, &mut path);
    }

    pub fn witness(&self) -> Ray {
        Ray::new(self)
    }
}

impl GroupNode {
    /// Assign the ID of this node, this will recursively update the ID of all the child nodes.
    #[inline]
    fn assign_id_recursive(
        &mut self,
        id: NodeId,
        dependencies: &mut HashMap<TypeId, Vec<NodeId>>,
        path: &mut Vec<NodeId>,
    ) -> NodeId {
        match &mut self.data {
            GroupNodeInner::Fork(left, right) => {
                self.id = id;
                path.push(self.id);
                let next_id = left.assign_id_recursive(id + 1, dependencies, path);
                let next_id = right.assign_id_recursive(next_id, dependencies, path);
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
                let next_id = node.assign_id_recursive(id + 1, dependencies, path);
                path.pop();
                self.id = id;
                next_id
            }
            _ => {
                self.id = id;
                id + 1
            }
        }
    }

    fn witness<'r>(&'r self, ray: &mut Ray<'r>) -> HashTree<'r> {
        if !ray.to_visit.contains(&self.id) {
            return Pruned(self.root_hash(ray));
        }

        match &self.data {
            GroupNodeInner::Empty => HashTree::Empty,
            GroupNodeInner::Fork(left, right) => {
                let l_tree = left.witness(ray);
                let r_tree = right.witness(ray);
                HashTree::Fork(Box::new(ForkInner(l_tree, r_tree)))
            }
            GroupNodeInner::Labeled(label, n) => {
                let tree = n.witness(ray);
                HashTree::Labeled(label.as_bytes(), Box::new(tree))
            }
            GroupNodeInner::Leaf(tid) => ray.interests.remove(tid).unwrap(),
        }
    }

    fn root_hash(&self, ray: &Ray) -> Hash {
        match &self.data {
            GroupNodeInner::Empty => HashTree::Empty.reconstruct(),
            GroupNodeInner::Fork(left, right) => {
                fork_hash(&left.root_hash(ray), &right.root_hash(ray))
            }
            GroupNodeInner::Labeled(label, node) => {
                let hash = node.root_hash(ray);
                labeled_hash(label.as_bytes(), &hash)
            }
            GroupNodeInner::Leaf(id) => ray.group.data.get(id).unwrap().root_hash(),
        }
    }
}

impl<'a> Ray<'a> {
    fn new(group: &'a Group) -> Self {
        Self {
            group,
            to_visit: HashSet::with_capacity(16),
            interests: HashMap::with_capacity(8),
        }
    }

    pub fn build(mut self) -> HashTree<'a> {
        self.group.root.witness(&mut self)
    }

    pub fn full<T: GroupLeaf + 'static>(mut self) -> Self {
        let tid = TypeId::of::<T>();

        for dep in self.group.dependencies.get(&tid).unwrap() {
            self.to_visit.insert(*dep);
        }

        let tree = self.group.data.get(&tid).unwrap().as_hash_tree();
        self.interests.insert(tid, tree);

        self
    }

    pub fn partial<T: GroupLeaf + 'static, F: FnOnce(&T) -> HashTree>(mut self, f: F) -> Self {
        let tid = TypeId::of::<T>();

        for dep in self.group.dependencies.get(&tid).unwrap() {
            self.to_visit.insert(*dep);
        }

        let data = self.group.data.get(&tid).unwrap();
        let tree = f(data.downcast_ref().unwrap());
        self.interests.insert(tid, tree);

        self
    }
}

pub trait GroupLeaf: Any + AsHashTree {}
impl<T: Any + AsHashTree> GroupLeaf for T {}

impl dyn GroupLeaf {
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        if self.type_id() == TypeId::of::<T>() {
            unsafe { Some(&*(self as *const dyn GroupLeaf as *const T)) }
        } else {
            None
        }
    }
}

#[test]
fn xxx() {
    let mut map = Map::<String, i8>::new();
    map.insert("X".to_string(), 17);

    println!("Hash : {}", hex::encode(map.root_hash()));

    let data: Box<dyn GroupLeaf> = Box::new(map);

    let as_map = data.downcast_ref::<Map<String, i8>>();
    println!("As map: {:?}", as_map);
}

#[test]
fn yyy() {
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
