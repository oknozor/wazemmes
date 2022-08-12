
use std::collections::hash_map::{Iter};
use std::collections::HashMap;

use crate::shell::container::ContainerRef;
use crate::shell::node::Node;
use crate::shell::window::WindowWarp;

#[derive(Debug, Default)]
pub struct NodeMap {
    // The node map
    pub items: HashMap<u32, Node>,
    // Node ids by their drawing order
    pub spine: Vec<u32>,
    // Store the id of the focused window
    pub focus_idx: Option<usize>,
}

impl NodeMap {
    pub fn iter_spine(&self) -> impl Iterator<Item=(usize, &u32, &Node)> {
        self.spine.iter().enumerate().map(|(idx, id)| {
            let node = self.items.get(id).unwrap();
            (idx, id, node)
        })
    }

    pub fn iter_windows(&self) -> impl Iterator<Item=&WindowWarp> {
        self.items.values().filter_map(|node| match node {
            Node::Container(_) => None,
            Node::Window(w) => Some(w),
        })
    }

    pub fn iter_containers(&self) -> impl Iterator<Item=&ContainerRef> {
        self.items.values().filter_map(|node| match node {
            Node::Container(c) => Some(c),
            Node::Window(_) => None,
        })
    }

    pub fn window_count(&self) -> i32 {
        self.iter_windows().count() as i32
    }

    pub fn container_count(&self) -> i32 {
        self.iter_containers().count() as i32
    }

    pub fn drain_containers(&mut self) -> Vec<(u32, Node)> {
        let ids: Vec<u32> = self.items
            .iter()
            .filter(|(_k, v)| v.is_container())
            .map(|(id, _n)| id)
            .cloned()
            .collect();

        let mut drained = vec![];

        for id in ids {
            self.spine.drain_filter(|id_| id == *id_);
            let node = self.items.remove(&id).unwrap();
            drained.push((id, node))
        }

        drained
    }

    pub fn drain_all(&mut self) -> Vec<(u32, Node)> {
        let mut drained = vec![];
        for id in &self.spine {
            let node = self.items.remove(id).unwrap();
            drained.push((*id, node))
        }

        drained
    }

    pub fn extend(&mut self, other: Vec<(u32, Node)>) {
        let ids: Vec<u32> = other.iter().map(|(id, _)| *id)
            .collect();
        self.spine.extend_from_slice(ids.as_slice());
        self.items.extend(other)
    }

    pub fn contains(&self, id: &u32) -> bool {
        self.spine.contains(id)
    }

    pub fn has_container(&self) -> bool {
        self.items.iter().any(|(_i, c)| c.is_container())
    }

    pub fn has_window(&self) -> bool {
        self.items.iter().any(|(_i, c)| !c.is_container())
    }

    pub fn get(&self, id: &u32) -> Option<&Node> {
        self.items.get(id)
    }

    pub fn insert(&mut self, id: u32, node: Node) {
        let new_focus = if let Some(focus) = self.focus_idx {
            focus + 1
        } else {
            0
        };

        if !node.is_container() {
            self.focus_idx = Some(new_focus);
        }

        self.spine.insert(new_focus, id);
        self.items.insert(id, node);
    }

    pub fn remove(&mut self, id: &u32) -> Option<Node> {
        self.remove_from_spine(id)
            .and_then(|id| self.items.remove(&id))
    }

    pub fn len(&self) -> usize {
        self.spine.len()
    }

    pub fn iter(&self) -> Iter<'_, u32, Node> {
        self.items.iter()
    }

    fn remove_from_spine(&mut self, id: &u32) -> Option<u32> {
        // Find the matching id in spine
        let spine_part = {
            let parts = self.spine
                .iter()
                .enumerate()
                .find(|(_idx, id_)| *id_ == id);

            parts.map(|(idx, id)| (idx, *id))
        };

        if let Some((idx, id)) = spine_part {
            self.spine.remove(idx);

            if self.spine.is_empty() {
                self.focus_idx = None
            } else {
                self.focus_idx = self.spine[..idx].iter().enumerate().rfind(|(_idx, id)| {
                    matches!(self.items.get(id), Some(Node::Window(_)))
                }).map(|(idx, _)| idx);
            }
            Some(id)
        } else {
            None
        }
    }

    pub fn set_focus(&mut self, id: u32) {
        self.focus_idx = self.spine.iter()
            .enumerate()
            .find(|(_, id_)| **id_ == id)
            .map(|(idx, _)| idx);
    }

    pub fn get_focused(&self) -> Option<&u32> {
        self.focus_idx.and_then(|idx| self.spine.get(idx))
    }
}