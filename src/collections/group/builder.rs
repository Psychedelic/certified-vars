use super::{Group, GroupLeaf, GroupNode, GroupNodeInner};
use crate::Map;
use std::any::{type_name, TypeId};
use std::collections::{BTreeMap, HashMap, VecDeque};

pub struct GroupBuilder {
    root: GroupBuilderNode,
    data: HashMap<TypeId, Box<dyn GroupLeaf>>,
}

enum GroupBuilderNode {
    Directory {
        children: BTreeMap<String, Box<GroupBuilderNode>>,
    },
    Leaf {
        tid: TypeId,
    },
}

impl GroupBuilder {
    pub fn new() -> Self {
        Self {
            root: GroupBuilderNode::Directory {
                children: BTreeMap::new(),
            },
            data: HashMap::new(),
        }
    }

    pub fn insert<T: GroupLeaf, C: Into<String>, P: IntoIterator<Item = C>>(
        mut self,
        path: P,
        data: T,
    ) -> Self {
        let path = path
            .into_iter()
            .map(|x| x.into())
            .collect::<VecDeque<String>>();

        let tid = TypeId::of::<T>();

        if self.data.insert(tid, Box::new(data)).is_some() {
            panic!("Type '{}' is already used in the group.", type_name::<T>())
        }

        self.root.insert(path, tid);

        self
    }

    #[must_use = "The constructed group must be used."]
    pub fn build(self) -> Group {
        let mut group = Group {
            root: self.root.build(),
            data: self.data,
            dependencies: Default::default(),
        };

        group.init();

        group
    }
}

impl GroupBuilderNode {
    pub fn insert(&mut self, mut path: VecDeque<String>, tid: TypeId) {
        if let GroupBuilderNode::Directory { children } = self {
            if path.len() == 1 {
                let name = path.pop_back().unwrap();

                let leaf = GroupBuilderNode::Leaf { tid };

                children
                    .entry(name.clone())
                    .and_modify(|_| panic!("Path is already used."))
                    .or_insert(Box::new(leaf));

                return;
            }
            let dir_name = path.pop_front().unwrap();

            children
                .entry(dir_name.clone())
                .or_insert_with(|| {
                    Box::new(GroupBuilderNode::Directory {
                        children: BTreeMap::new(),
                    })
                })
                .insert(path, tid);
            return;
        }

        panic!("Can not insert to a leaf node.");
    }

    pub fn build(self) -> GroupNode {
        match self {
            GroupBuilderNode::Directory { mut children } => {
                let mut children = children
                    .into_iter()
                    .map(|(k, v)| GroupNode {
                        id: 0,
                        data: GroupNodeInner::Labeled(k, Box::new(v.build())),
                    })
                    .collect::<VecDeque<_>>();

                // Create a semi-balanced binary tree out of the child nodes.
                // (Using `reduce` will not generate a balanced tree.)
                while children.len() > 1 {
                    let mut new_children = VecDeque::with_capacity(children.len() / 2);

                    while children.len() > 1 {
                        let a = Box::new(children.pop_front().unwrap());
                        let b = Box::new(children.pop_front().unwrap());
                        new_children.push_back(GroupNode {
                            id: 0,
                            data: GroupNodeInner::Fork(a, b),
                        })
                    }

                    if let Some(last) = children.pop_front() {
                        new_children.push_back(last);
                    }

                    children = new_children;
                }

                children.pop_front().unwrap()
            }
            GroupBuilderNode::Leaf { tid } => GroupNode {
                id: 0,
                data: GroupNodeInner::Leaf(tid),
            },
        }
    }
}

#[test]
fn xxx() {
    let map = Map::<String, i8>::new();
    let mut group = GroupBuilder::new()
        .insert(["a", "x"], map)
        .insert(["a", "y"], 17u8)
        .insert(["a", "z"], 32u32)
        .build();

    println!("{:#?}", group.root);
    println!("{:?}", group.get_mut::<u8>());
}
