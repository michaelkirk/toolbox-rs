use crate::edge::Edge;
use crate::edge::InputEdge;
use crate::graph::{Graph, NodeID};
use crate::static_graph::StaticGraph;
use bitvec::vec::BitVec;
use core::cmp::min;
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeCapacity {
    pub capacity: i32,
}

impl EdgeCapacity {
    pub fn new(capacity: i32) -> EdgeCapacity {
        EdgeCapacity { capacity }
    }
}

pub struct Dinic<'a> {
    residual_graph: StaticGraph<EdgeCapacity>,
    max_flow: i32,
    finished: bool,
    level: Vec<usize>,
    parents: Vec<NodeID>,
    stack: Vec<(NodeID, i32)>,
    dfs_count: usize,
    bfs_count: usize,
    queue: VecDeque<NodeID>,
    sources: &'a [NodeID],
    targets: &'a [NodeID],
}

impl<'a> Dinic<'a> {
    // todo(dl): add closure parameter to derive edge data
    pub fn from_generic_edge_list(
        input_edges: Vec<impl Edge<ID = NodeID>>,
        sources: &'a [NodeID],
        targets: &'a [NodeID],
    ) -> Self {
        let edge_list: Vec<InputEdge<EdgeCapacity>> = input_edges
            .into_iter()
            .map(|edge| InputEdge {
                source: edge.source(),
                target: edge.target(),
                data: EdgeCapacity::new(1),
            })
            .collect();

        println!("created {} ff edges", edge_list.len());
        Dinic::from_edge_list(edge_list, sources, targets)
    }

    pub fn from_edge_list(
        mut edge_list: Vec<InputEdge<EdgeCapacity>>,
        sources: &'a [usize],
        targets: &'a [usize],
    ) -> Self {
        let number_of_edges = edge_list.len();

        println!("extending {} edges", edge_list.len());
        // blindly generate reverse edges for all edges with zero capacity
        edge_list.extend_from_within(..);
        edge_list.iter_mut().skip(number_of_edges).for_each(|edge| {
            edge.reverse();
            edge.data.capacity = 0;
        });
        println!("into {} edges", edge_list.len());

        // dedup-merge edge set, by using the following trick: not the dedup(.) call
        // below takes the second argument as mut. When deduping equivalent values
        // a and b, then a is accumulated onto b.
        edge_list.sort_unstable();
        edge_list.dedup_by(|a, mut b| {
            // edges a and b are assumed to be equivalent in the residual graph if
            // (and only if) they are parallel. In other words, this removes parallel
            // edges in the residual graph and accumulates capacities on the remaining
            // egde.
            let result = a.source == b.source && a.target == b.target;
            if result {
                b.data.capacity += a.data.capacity;
            }
            result
        });

        // at this point the edge set of the residual graph doesn't have any
        // duplicates anymore. note that this is fine, as we are looking to
        // compute a node partition.
        Self {
            residual_graph: StaticGraph::new(edge_list),
            max_flow: 0,
            finished: false,
            level: Vec::new(),
            parents: Vec::new(),
            stack: Vec::new(),
            dfs_count: 0,
            bfs_count: 0,
            queue: VecDeque::new(),
            sources,
            targets,
        }
    }

    pub fn run(&mut self, sources: &[NodeID], targets: &[NodeID]) {
        println!("sources: {}, targets {}", sources.len(), targets.len());

        let number_of_nodes = self.residual_graph.number_of_nodes();
        self.parents.resize(number_of_nodes, 0);
        self.level.resize(number_of_nodes, usize::MAX);
        self.queue.reserve(number_of_nodes);

        let mut flow = 0;
        loop {
            if !self.bfs() {
                // no path between sources and target possible anymore
                break;
            }
            while let Some(pushed) = self.dfs() {
                // incremental path in DFS found
                flow += pushed;
            }
        }
        self.max_flow = flow;
        self.finished = true;
    }

    fn bfs(&mut self) -> bool {
        let start = Instant::now();
        self.bfs_count += 1;
        // init
        self.level.fill(usize::MAX);
        self.queue.extend(self.sources.iter().copied());
        for s in self.sources {
            self.level[*s] = 0;
            // self.queue.push_back(*s);
        }
        for t in self.targets {
            self.level[*t] = usize::MAX - 1;
        }

        // label residual graph nodes in BFS order
        let mut found_path = false;
        while let Some(u) = self.queue.pop_front() {
            for edge in self.residual_graph.edge_range(u) {
                let edge_data = self.residual_graph.data(edge);
                let v = self.residual_graph.target(edge);
                if edge_data.capacity < 1 {
                    // no flow on this edge
                    continue;
                }
                if self.level[v] < usize::MAX - 1 {
                    // node already visited
                    continue;
                }
                let is_target = self.level[v] == usize::MAX - 1;
                self.level[v] = self.level[u] + 1;
                if is_target {
                    found_path = true;
                } else {
                    self.queue.push_back(v);
                }
            }
        }
        let duration = start.elapsed();
        println!("BFS took: {:?}", duration);

        found_path
    }

    fn dfs(&mut self) -> Option<i32> {
        let start = Instant::now();
        self.dfs_count += 1;
        self.stack.clear();
        // println!("DFS stack capacity: {}", self.stack.capacity());
        self.parents.fill(NodeID::MAX);

        let duration = start.elapsed();
        println!(" DFS init1: {:?}", duration);

        for u in self.sources {
            self.stack.push((*u, i32::MAX));
            self.parents[*u] = *u;
        }

        let duration = start.elapsed();
        println!(" DFS init2: {:?}", duration);

        for t in self.targets {
            self.parents[*t] = NodeID::MAX - 1;
        }

        let duration = start.elapsed();
        println!(" DFS init3: {:?}", duration);

        while let Some((node, flow)) = self.stack.pop() {
            for edge in self.residual_graph.edge_range(node) {
                let target = self.residual_graph.target(edge);
                if self.parents[target] < NodeID::MAX - 1 {
                    // target already in queue
                    continue;
                }
                if self.level[node] > self.level[target] {
                    // edge is not on a path in BFS tree
                    continue;
                }
                let available_capacity = self.residual_graph.data(edge).capacity;
                if available_capacity < 1 {
                    // no capacity to use on this edge
                    continue;
                }
                let is_parent = self.parents[target] == NodeID::MAX - 1;
                self.parents[target] = node;
                let flow = min(flow, available_capacity);
                if is_parent {
                    // reached a target. Unpack path, assign flow
                    let mut v = target;
                    loop {
                        let u = self.parents[v];
                        if u == v {
                            break;
                        }
                        let fwd_edge = self.residual_graph.find_edge(u, v).unwrap();
                        self.residual_graph.data_mut(fwd_edge).capacity -= flow;
                        let rev_edge = self.residual_graph.find_edge(v, u).unwrap();
                        self.residual_graph.data_mut(rev_edge).capacity += flow;
                        v = u;
                    }
                    let duration = start.elapsed();
                    println!("DFS took: {:?} (success)", duration);
                    return Some(flow);
                } else {
                    self.stack.push((target, flow));
                }
            }
        }

        let duration = start.elapsed();
        println!("DFS took: {:?} (unsuccessful)", duration);
        None
    }

    pub fn max_flow(&self) -> Result<i32, String> {
        if !self.finished {
            return Err("Assigment was not computed.".to_string());
        }
        println!("DFS: {}, BFS: {}", self.dfs_count, self.bfs_count);
        Ok(self.max_flow)
    }

    pub fn assignment(&self, sources: &[NodeID]) -> Result<BitVec, String> {
        if !self.finished {
            return Err("Assigment was not computed.".to_string());
        }

        // run a reachability analysis
        let mut reachable = BitVec::with_capacity(self.residual_graph.number_of_nodes());
        reachable.resize(self.residual_graph.number_of_nodes(), false);
        let mut stack: Vec<usize> = sources.iter().copied().collect();
        while let Some(node) = stack.pop() {
            // TODO: looks like this following is superflous work?
            if *reachable.get(node as usize).unwrap() {
                continue;
            }
            reachable.set(node as usize, true);
            for edge in self.residual_graph.edge_range(node) {
                let target = self.residual_graph.target(edge);
                let reached = reachable.get(target as usize).unwrap();
                if !reached && self.residual_graph.data(edge).capacity > 0 {
                    stack.push(self.residual_graph.target(edge));
                }
            }
        }
        Ok(reachable)
    }
}

#[cfg(test)]
mod tests {

    use crate::dinic::Dinic;
    use crate::dinic::EdgeCapacity;
    use crate::edge::InputEdge;
    use bitvec::bits;
    use bitvec::prelude::Lsb0;

    #[test]
    fn max_flow_clr() {
        let edges = vec![
            InputEdge::new(0, 1, EdgeCapacity::new(16)),
            InputEdge::new(0, 2, EdgeCapacity::new(13)),
            InputEdge::new(1, 2, EdgeCapacity::new(10)),
            InputEdge::new(1, 3, EdgeCapacity::new(12)),
            InputEdge::new(2, 1, EdgeCapacity::new(4)),
            InputEdge::new(2, 4, EdgeCapacity::new(14)),
            InputEdge::new(3, 2, EdgeCapacity::new(9)),
            InputEdge::new(3, 5, EdgeCapacity::new(20)),
            InputEdge::new(4, 3, EdgeCapacity::new(7)),
            InputEdge::new(4, 5, EdgeCapacity::new(4)),
        ];

        let sources = [0];
        let targets = [5];
        let mut max_flow_solver = Dinic::from_edge_list(edges, &sources, &targets);
        max_flow_solver.run(&sources, &targets);

        // it's OK to expect the solver to have run
        let max_flow = max_flow_solver
            .max_flow()
            .expect("max flow computation did not run");
        assert_eq!(23, max_flow);

        // it's OK to expect the solver to have run
        let assignment = max_flow_solver
            .assignment(&sources)
            .expect("assignment computation did not run");

        assert_eq!(assignment, bits![1, 1, 1, 0, 1, 0]);
    }

    #[test]
    fn max_flow_clr_multi_target_set() {
        let edges = vec![
            InputEdge::new(0, 1, EdgeCapacity::new(16)),
            InputEdge::new(0, 2, EdgeCapacity::new(13)),
            InputEdge::new(1, 2, EdgeCapacity::new(10)),
            InputEdge::new(1, 3, EdgeCapacity::new(12)),
            InputEdge::new(2, 1, EdgeCapacity::new(4)),
            InputEdge::new(2, 4, EdgeCapacity::new(14)),
            InputEdge::new(3, 2, EdgeCapacity::new(9)),
            InputEdge::new(3, 5, EdgeCapacity::new(20)),
            InputEdge::new(4, 3, EdgeCapacity::new(7)),
            InputEdge::new(4, 5, EdgeCapacity::new(4)),
            InputEdge::new(5, 6, EdgeCapacity::new(1)),
            InputEdge::new(6, 1, EdgeCapacity::new(41)),
        ];

        let sources = [0];
        let targets = [5, 6];
        let mut max_flow_solver = Dinic::from_edge_list(edges, &sources, &targets);
        max_flow_solver.run(&sources, &targets);

        // it's OK to expect the solver to have run
        let max_flow = max_flow_solver
            .max_flow()
            .expect("max flow computation did not run");
        assert_eq!(23, max_flow);

        // it's OK to expect the solver to have run
        let assignment = max_flow_solver
            .assignment(&sources)
            .expect("assignment computation did not run");

        assert_eq!(assignment, bits![1, 1, 1, 0, 1, 0, 0]);
    }

    #[test]
    fn max_flow_ita() {
        let edges = vec![
            InputEdge::new(0, 1, EdgeCapacity::new(5)),
            InputEdge::new(0, 4, EdgeCapacity::new(7)),
            InputEdge::new(0, 5, EdgeCapacity::new(6)),
            InputEdge::new(1, 2, EdgeCapacity::new(4)),
            InputEdge::new(1, 7, EdgeCapacity::new(3)),
            InputEdge::new(4, 7, EdgeCapacity::new(4)),
            InputEdge::new(4, 6, EdgeCapacity::new(1)),
            InputEdge::new(5, 6, EdgeCapacity::new(5)),
            InputEdge::new(2, 3, EdgeCapacity::new(3)),
            InputEdge::new(7, 3, EdgeCapacity::new(7)),
            InputEdge::new(6, 7, EdgeCapacity::new(1)),
            InputEdge::new(6, 3, EdgeCapacity::new(6)),
        ];

        let sources = [0];
        let targets = [3];
        let mut max_flow_solver = Dinic::from_edge_list(edges, &sources, &targets);
        max_flow_solver.run(&sources, &targets);

        // it's OK to expect the solver to have run
        let max_flow = max_flow_solver
            .max_flow()
            .expect("max flow computation did not run");
        assert_eq!(15, max_flow);

        // it's OK to expect the solver to have run
        let assignment = max_flow_solver
            .assignment(&sources)
            .expect("assignment computation did not run");
        assert_eq!(assignment, bits![1, 0, 0, 0, 1, 1, 0, 0]);
    }

    #[test]
    fn max_flow_yt() {
        let edges = vec![
            InputEdge::new(9, 0, EdgeCapacity::new(5)),
            InputEdge::new(9, 1, EdgeCapacity::new(10)),
            InputEdge::new(9, 2, EdgeCapacity::new(15)),
            InputEdge::new(0, 3, EdgeCapacity::new(10)),
            InputEdge::new(1, 0, EdgeCapacity::new(15)),
            InputEdge::new(1, 4, EdgeCapacity::new(20)),
            InputEdge::new(2, 5, EdgeCapacity::new(25)),
            InputEdge::new(3, 4, EdgeCapacity::new(25)),
            InputEdge::new(3, 6, EdgeCapacity::new(10)),
            InputEdge::new(4, 2, EdgeCapacity::new(5)),
            InputEdge::new(4, 7, EdgeCapacity::new(30)),
            InputEdge::new(5, 7, EdgeCapacity::new(20)),
            InputEdge::new(5, 8, EdgeCapacity::new(10)),
            InputEdge::new(7, 8, EdgeCapacity::new(15)),
            InputEdge::new(6, 10, EdgeCapacity::new(5)),
            InputEdge::new(7, 10, EdgeCapacity::new(15)),
            InputEdge::new(8, 10, EdgeCapacity::new(10)),
        ];

        let sources = [9];
        let targets = [10];
        let mut max_flow_solver = Dinic::from_edge_list(edges, &sources, &targets);
        max_flow_solver.run(&sources, &targets);

        // it's OK to expect the solver to have run
        let max_flow = max_flow_solver
            .max_flow()
            .expect("max flow computation did not run");
        assert_eq!(30, max_flow);

        // it's OK to expect the solver to have run
        let assignment = max_flow_solver
            .assignment(&sources)
            .expect("assignment computation did not run");
        assert_eq!(assignment, bits![0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0]);
    }

    #[test]
    fn max_flow_ff() {
        let edges = vec![
            InputEdge::new(0, 1, EdgeCapacity::new(7)),
            InputEdge::new(0, 2, EdgeCapacity::new(3)),
            InputEdge::new(1, 2, EdgeCapacity::new(1)),
            InputEdge::new(1, 3, EdgeCapacity::new(6)),
            InputEdge::new(2, 4, EdgeCapacity::new(8)),
            InputEdge::new(3, 5, EdgeCapacity::new(2)),
            InputEdge::new(3, 2, EdgeCapacity::new(3)),
            InputEdge::new(4, 3, EdgeCapacity::new(2)),
            InputEdge::new(4, 5, EdgeCapacity::new(8)),
        ];

        let sources = [0];
        let targets = [5];
        let mut max_flow_solver = Dinic::from_edge_list(edges, &sources, &targets);
        max_flow_solver.run(&sources, &targets);

        // it's OK to expect the solver to have run
        let max_flow = max_flow_solver
            .max_flow()
            .expect("max flow computation did not run");
        assert_eq!(9, max_flow);

        // it's OK to expect the solver to have run
        let assignment = max_flow_solver
            .assignment(&sources)
            .expect("assignment computation did not run");
        assert_eq!(assignment, bits![1, 1, 0, 1, 0, 0]);
    }

    #[test]
    #[should_panic]
    fn flow_not_computed() {
        let edges = vec![
            InputEdge::new(0, 1, EdgeCapacity::new(7)),
            InputEdge::new(0, 2, EdgeCapacity::new(3)),
            InputEdge::new(1, 2, EdgeCapacity::new(1)),
            InputEdge::new(1, 3, EdgeCapacity::new(6)),
            InputEdge::new(2, 4, EdgeCapacity::new(8)),
            InputEdge::new(3, 5, EdgeCapacity::new(2)),
            InputEdge::new(3, 2, EdgeCapacity::new(3)),
            InputEdge::new(4, 3, EdgeCapacity::new(2)),
            InputEdge::new(4, 5, EdgeCapacity::new(8)),
        ];

        // the expect(.) call is being tested
        Dinic::from_edge_list(edges, &[], &[])
            .max_flow()
            .expect("max flow computation did not run");
    }

    #[test]
    #[should_panic]
    fn assignment_not_computed() {
        let edges = vec![
            InputEdge::new(0, 1, EdgeCapacity::new(7)),
            InputEdge::new(0, 2, EdgeCapacity::new(3)),
            InputEdge::new(1, 2, EdgeCapacity::new(1)),
            InputEdge::new(1, 3, EdgeCapacity::new(6)),
            InputEdge::new(2, 4, EdgeCapacity::new(8)),
            InputEdge::new(3, 5, EdgeCapacity::new(2)),
            InputEdge::new(3, 2, EdgeCapacity::new(3)),
            InputEdge::new(4, 3, EdgeCapacity::new(2)),
            InputEdge::new(4, 5, EdgeCapacity::new(8)),
        ];

        // the expect(.) call is being tested
        Dinic::from_edge_list(edges, &[], &[])
            .assignment(&[0])
            .expect("assignment computation did not run");
    }
}
