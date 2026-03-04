// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{MinimalModelKey, StateFormula};
use crate::location::LocationId;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

static NEXT_MANAGER_ID: AtomicU64 = AtomicU64::new(1);

/// Node identifier in the hash-consed DAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct NodeId(usize);

impl NodeId {
    const BOT: Self = Self(0);
    const TOP: Self = Self(1);

    #[inline]
    const fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Node {
    Top,
    Bot,
    Var(LocationId),
    And(Vec<NodeId>),
    Or(Vec<NodeId>),
}

#[derive(Debug, Default)]
struct DagStore {
    nodes: Vec<Node>,
    intern: HashMap<Node, NodeId>,
}

/// Manager for [`DagStateFormula`] nodes.
#[derive(Debug)]
pub struct DagStateFormulaManager {
    manager_id: u64,
    store: Mutex<DagStore>,
}

impl DagStateFormulaManager {
    /// Create a new DAG manager.
    ///
    /// Nodes `⊥` and `⊤` are pre-interned in deterministic order.
    pub fn new() -> Arc<Self> {
        let manager = Arc::new(Self {
            manager_id: NEXT_MANAGER_ID.fetch_add(1, Ordering::Relaxed),
            store: Mutex::new(DagStore::default()),
        });

        {
            let mut store = manager.lock_store();
            store.nodes.push(Node::Bot);
            store.intern.insert(Node::Bot, NodeId::BOT);

            store.nodes.push(Node::Top);
            store.intern.insert(Node::Top, NodeId::TOP);
        }

        manager
    }

    #[inline]
    fn id(&self) -> u64 {
        self.manager_id
    }

    fn lock_store(&self) -> MutexGuard<'_, DagStore> {
        match self.store.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn node(&self, id: NodeId) -> Node {
        let store = self.lock_store();
        store.nodes.get(id.as_usize()).cloned().unwrap_or(Node::Bot)
    }

    fn intern_node(&self, node: Node) -> NodeId {
        let mut store = self.lock_store();
        if let Some(id) = store.intern.get(&node).copied() {
            return id;
        }

        let id = NodeId(store.nodes.len());
        store.nodes.push(node.clone());
        store.intern.insert(node, id);
        id
    }

    fn import_into(target: &Arc<Self>, cfg: &DagStateFormula) -> NodeId {
        if target.id() == cfg.manager().id() {
            return cfg.id;
        }

        let mut memo = HashMap::new();
        Self::import_node_into(target, cfg, cfg.id, &mut memo)
    }

    fn import_node_into(
        target: &Arc<Self>,
        cfg: &DagStateFormula,
        source_id: NodeId,
        memo: &mut HashMap<NodeId, NodeId>,
    ) -> NodeId {
        if let Some(mapped) = memo.get(&source_id).copied() {
            return mapped;
        }

        let source_node = cfg.manager().node(source_id);
        let mapped = match source_node {
            Node::Top => NodeId::TOP,
            Node::Bot => NodeId::BOT,
            Node::Var(v) => target.intern_node(Node::Var(v)),
            Node::And(children) => {
                let imported = children
                    .into_iter()
                    .map(|child| Self::import_node_into(target, cfg, child, memo));
                target.normalize_and_ids(imported)
            }
            Node::Or(children) => {
                let imported = children
                    .into_iter()
                    .map(|child| Self::import_node_into(target, cfg, child, memo));
                target.normalize_or_ids(imported)
            }
        };

        memo.insert(source_id, mapped);
        mapped
    }

    fn normalize_and_ids(&self, ids: impl IntoIterator<Item = NodeId>) -> NodeId {
        let mut stack: Vec<NodeId> = ids.into_iter().collect();
        let mut children = Vec::new();

        while let Some(id) = stack.pop() {
            if id == NodeId::BOT {
                return NodeId::BOT;
            }
            if id == NodeId::TOP {
                continue;
            }

            match self.node(id) {
                Node::And(sub) => stack.extend(sub),
                _ => children.push(id),
            }
        }

        if children.is_empty() {
            return NodeId::TOP;
        }

        children.sort_unstable();
        children.dedup();

        if children.len() == 1 {
            return children[0];
        }

        self.intern_node(Node::And(children))
    }

    fn normalize_or_ids(&self, ids: impl IntoIterator<Item = NodeId>) -> NodeId {
        let mut stack: Vec<NodeId> = ids.into_iter().collect();
        let mut children = Vec::new();

        while let Some(id) = stack.pop() {
            if id == NodeId::TOP {
                return NodeId::TOP;
            }
            if id == NodeId::BOT {
                continue;
            }

            match self.node(id) {
                Node::Or(sub) => stack.extend(sub),
                _ => children.push(id),
            }
        }

        if children.is_empty() {
            return NodeId::BOT;
        }

        children.sort_unstable();
        children.dedup();

        if children.len() == 1 {
            return children[0];
        }

        self.intern_node(Node::Or(children))
    }

    fn format_node(&self, id: NodeId) -> String {
        match self.node(id) {
            Node::Top => "⊤".to_string(),
            Node::Bot => "⊥".to_string(),
            Node::Var(v) => format!("loc({v})"),
            Node::And(children) => {
                let rendered = children
                    .iter()
                    .map(|child| self.format_node(*child))
                    .collect::<Vec<_>>()
                    .join(" & ");
                format!("({rendered})")
            }
            Node::Or(children) => {
                let rendered = children
                    .iter()
                    .map(|child| self.format_node(*child))
                    .collect::<Vec<_>>()
                    .join(" | ");
                format!("({rendered})")
            }
        }
    }

    fn dnf_node(&self, id: NodeId) -> Vec<Vec<LocationId>> {
        match self.node(id) {
            Node::Bot => vec![],
            Node::Top => vec![vec![]],
            Node::Var(v) => vec![vec![v]],
            Node::And(children) => {
                let mut result: Vec<Vec<LocationId>> = vec![vec![]];
                for child in children {
                    let child_dnf = self.dnf_node(child);
                    let mut new_result = Vec::new();
                    for conj in &result {
                        for child_conj in &child_dnf {
                            let mut merged = conj.clone();
                            merged.extend(child_conj.iter().cloned());
                            merged.sort();
                            merged.dedup();
                            new_result.push(merged);
                        }
                    }
                    result = new_result;
                }
                result
            }
            Node::Or(children) => {
                let mut result = Vec::new();
                for child in children {
                    result.extend(self.dnf_node(child));
                }
                result
            }
        }
    }

    fn semantic_key_node(
        &self,
        id: NodeId,
        memo: &mut HashMap<NodeId, MinimalModelKey<LocationId>>,
    ) -> MinimalModelKey<LocationId> {
        if let Some(key) = memo.get(&id) {
            return key.clone();
        }

        let key = match self.node(id) {
            Node::Top => MinimalModelKey::top(),
            Node::Bot => MinimalModelKey::bot(),
            Node::Var(v) => MinimalModelKey::var(v),
            Node::And(children) => MinimalModelKey::intersection_all(
                children
                    .into_iter()
                    .map(|child| self.semantic_key_node(child, memo)),
            ),
            Node::Or(children) => MinimalModelKey::union_all(
                children
                    .into_iter()
                    .map(|child| self.semantic_key_node(child, memo)),
            ),
        };

        memo.insert(id, key.clone());
        key
    }
}

/// Hash-consed DAG-based state-formula representation.
#[derive(Clone)]
pub struct DagStateFormula {
    id: NodeId,
    mgr: Arc<DagStateFormulaManager>,
}

impl DagStateFormula {
    fn new(id: NodeId, mgr: Arc<DagStateFormulaManager>) -> Self {
        Self { id, mgr }
    }

    fn manager_id(&self) -> u64 {
        self.mgr.id()
    }

    /// Convert this formula to disjunctive normal form.
    ///
    /// - `vec![]` represents `⊥`.
    /// - `vec![vec![]]` represents `⊤`.
    pub fn to_dnf(&self) -> Vec<Vec<LocationId>> {
        self.mgr.dnf_node(self.id)
    }

    /// Canonical semantic key based on minimal satisfying location sets.
    pub fn semantic_key(&self) -> MinimalModelKey<LocationId> {
        <Self as StateFormula>::semantic_key(self)
    }
}

impl PartialEq for DagStateFormula {
    fn eq(&self, other: &Self) -> bool {
        self.manager_id() == other.manager_id() && self.id == other.id
    }
}

impl Eq for DagStateFormula {}

impl Hash for DagStateFormula {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.manager_id().hash(state);
        self.id.hash(state);
    }
}

impl Debug for DagStateFormula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DagStateFormula")
            .field("manager_id", &self.manager_id())
            .field("id", &self.id)
            .field("expr", &self.to_string())
            .finish()
    }
}

impl Display for DagStateFormula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mgr.format_node(self.id))
    }
}

impl StateFormula for DagStateFormula {
    type Var = LocationId;
    type Manager = Arc<DagStateFormulaManager>;

    fn top(mgr: &Self::Manager) -> Self {
        Self::new(NodeId::TOP, Arc::clone(mgr))
    }

    fn bot(mgr: &Self::Manager) -> Self {
        Self::new(NodeId::BOT, Arc::clone(mgr))
    }

    fn var(mgr: &Self::Manager, v: Self::Var) -> Self {
        let id = mgr.intern_node(Node::Var(v));
        Self::new(id, Arc::clone(mgr))
    }

    fn and(mgr: &Self::Manager, terms: impl IntoIterator<Item = Self>) -> Self {
        let imported_ids = terms
            .into_iter()
            .map(|term| DagStateFormulaManager::import_into(mgr, &term));
        let id = mgr.normalize_and_ids(imported_ids);
        Self::new(id, Arc::clone(mgr))
    }

    fn or(mgr: &Self::Manager, terms: impl IntoIterator<Item = Self>) -> Self {
        let imported_ids = terms
            .into_iter()
            .map(|term| DagStateFormulaManager::import_into(mgr, &term));
        let id = mgr.normalize_or_ids(imported_ids);
        Self::new(id, Arc::clone(mgr))
    }

    fn size(&self) -> usize {
        let mut visited = HashSet::new();
        let mut stack = vec![self.id];
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            match self.mgr.node(id) {
                Node::And(children) | Node::Or(children) => stack.extend(children),
                Node::Top | Node::Bot | Node::Var(_) => {}
            }
        }
        visited.len()
    }

    fn vars(&self) -> Vec<Self::Var> {
        let mut seen_nodes = HashSet::new();
        let mut vars = BTreeSet::new();
        let mut stack = vec![self.id];

        while let Some(id) = stack.pop() {
            if !seen_nodes.insert(id) {
                continue;
            }

            match self.mgr.node(id) {
                Node::Var(v) => {
                    vars.insert(v);
                }
                Node::And(children) | Node::Or(children) => stack.extend(children),
                Node::Top | Node::Bot => {}
            }
        }

        vars.into_iter().collect()
    }

    fn manager(&self) -> &Self::Manager {
        &self.mgr
    }

    fn substitute(mgr: &Self::Manager, f: &Self, mut sub: impl FnMut(Self::Var) -> Self) -> Self {
        fn rewrite_node(
            target_mgr: &Arc<DagStateFormulaManager>,
            source_cfg: &DagStateFormula,
            node_id: NodeId,
            memo: &mut HashMap<NodeId, NodeId>,
            sub: &mut impl FnMut(LocationId) -> DagStateFormula,
        ) -> NodeId {
            if let Some(mapped) = memo.get(&node_id).copied() {
                return mapped;
            }

            let mapped = match source_cfg.mgr.node(node_id) {
                Node::Top => NodeId::TOP,
                Node::Bot => NodeId::BOT,
                Node::Var(v) => {
                    let replacement = sub(v);
                    DagStateFormulaManager::import_into(target_mgr, &replacement)
                }
                Node::And(children) => {
                    let rewritten = children
                        .into_iter()
                        .map(|child| rewrite_node(target_mgr, source_cfg, child, memo, sub));
                    target_mgr.normalize_and_ids(rewritten)
                }
                Node::Or(children) => {
                    let rewritten = children
                        .into_iter()
                        .map(|child| rewrite_node(target_mgr, source_cfg, child, memo, sub));
                    target_mgr.normalize_or_ids(rewritten)
                }
            };

            memo.insert(node_id, mapped);
            mapped
        }

        let mut memo = HashMap::new();
        let rewritten_root = rewrite_node(mgr, f, f.id, &mut memo, &mut sub);
        DagStateFormula::new(rewritten_root, Arc::clone(mgr))
    }

    fn eval_bool(f: &Self, mut val: impl FnMut(Self::Var) -> bool) -> bool {
        fn eval_node(
            cfg: &DagStateFormula,
            node_id: NodeId,
            memo: &mut HashMap<NodeId, bool>,
            val: &mut impl FnMut(LocationId) -> bool,
        ) -> bool {
            if let Some(cached) = memo.get(&node_id).copied() {
                return cached;
            }

            let result = match cfg.mgr.node(node_id) {
                Node::Top => true,
                Node::Bot => false,
                Node::Var(v) => val(v),
                Node::And(children) => children
                    .into_iter()
                    .all(|child| eval_node(cfg, child, memo, val)),
                Node::Or(children) => children
                    .into_iter()
                    .any(|child| eval_node(cfg, child, memo, val)),
            };

            memo.insert(node_id, result);
            result
        }

        let mut memo = HashMap::new();
        eval_node(f, f.id, &mut memo, &mut val)
    }

    fn semantic_key(&self) -> MinimalModelKey<Self::Var> {
        let mut memo = HashMap::new();
        self.mgr.semantic_key_node(self.id, &mut memo)
    }
}
