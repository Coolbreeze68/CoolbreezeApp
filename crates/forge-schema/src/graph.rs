//! Graphe de dépendances entre colonnes calculées.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::ColumnRef;

/// Résultat du tri : ordre topologique et cycles détectés.
#[derive(Debug, Default)]
pub(crate) struct Sorted {
    pub order: Vec<ColumnRef>,
    /// Chaque cycle est donné dans l'ordre des dépendances, en revenant au point de départ.
    pub cycles: Vec<Vec<ColumnRef>>,
}

/// Trie les nœuds pour que chacun vienne après ses dépendances.
///
/// Seuls les nœuds présents comme clés sont ordonnés ; les dépendances vers des
/// colonnes non calculées sont ignorées. Le parcours suit l'ordre des clés pour
/// rester déterministe.
pub(crate) fn sort(edges: &BTreeMap<ColumnRef, BTreeSet<ColumnRef>>) -> Sorted {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        InProgress,
        Done,
    }

    fn visit<'a>(
        node: &'a ColumnRef,
        edges: &'a BTreeMap<ColumnRef, BTreeSet<ColumnRef>>,
        states: &mut BTreeMap<&'a ColumnRef, State>,
        stack: &mut Vec<&'a ColumnRef>,
        sorted: &mut Sorted,
    ) {
        match states.get(node) {
            Some(State::Done) => return,
            Some(State::InProgress) => {
                let start = stack.iter().position(|n| *n == node).unwrap_or(0);
                let mut cycle: Vec<ColumnRef> =
                    stack[start..].iter().map(|n| (*n).clone()).collect();
                cycle.push(node.clone());
                sorted.cycles.push(cycle);
                return;
            }
            None => {}
        }
        states.insert(node, State::InProgress);
        stack.push(node);
        for dep in edges[node].iter().filter(|d| edges.contains_key(*d)) {
            visit(dep, edges, states, stack, sorted);
        }
        stack.pop();
        states.insert(node, State::Done);
        sorted.order.push(node.clone());
    }

    let mut states = BTreeMap::new();
    let mut sorted = Sorted::default();
    for node in edges.keys() {
        visit(node, edges, &mut states, &mut Vec::new(), &mut sorted);
    }
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str) -> ColumnRef {
        ColumnRef::new("t", name)
    }

    fn graph(edges: &[(&str, &[&str])]) -> BTreeMap<ColumnRef, BTreeSet<ColumnRef>> {
        edges
            .iter()
            .map(|(from, to)| (col(from), to.iter().map(|t| col(t)).collect()))
            .collect()
    }

    #[test]
    fn orders_dependencies_first() {
        let sorted = sort(&graph(&[("c", &["b", "x"]), ("b", &["a"]), ("a", &[])]));
        assert!(sorted.cycles.is_empty());
        assert_eq!(sorted.order, vec![col("a"), col("b"), col("c")]);
    }

    #[test]
    fn detects_cycles() {
        let sorted = sort(&graph(&[("a", &["b"]), ("b", &["c"]), ("c", &["a"])]));
        assert_eq!(
            sorted.cycles,
            vec![vec![col("a"), col("b"), col("c"), col("a")]]
        );
    }

    #[test]
    fn detects_self_reference() {
        let sorted = sort(&graph(&[("a", &["a"])]));
        assert_eq!(sorted.cycles, vec![vec![col("a"), col("a")]]);
    }
}
