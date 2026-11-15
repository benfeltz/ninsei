//! The hop trail: the links a runner actually came along.

use ninsei_core::*;

#[test]
fn the_trail_is_the_path_you_took() {
    let d = Data::builtin().unwrap();
    for seed in 0..30 {
        let mut g = Game::new(d.clone(), seed);
        // Clear the way so the walk starts no fights or talks.
        for n in &mut g.net.nodes {
            if n.id != g.at {
                n.kind = NodeKind::Vault;
                n.cleared = false;
            }
        }
        g.net.orient(seed, 0);
        let mut path = vec![g.at];
        // Three hops deeper, along the last link, then one back.
        for _ in 0..3 {
            let Some(&to) = g.net.nodes[g.at].links.last() else {
                break;
            };
            g.apply(Action::Move(to));
            path.push(to);
        }
        let back = path[path.len() - 2];
        g.apply(Action::Move(back));
        assert_eq!(g.at, back);
        let mut trail = Vec::new();
        for n in &g.net.nodes {
            for &to in &n.links {
                if g.travelled(n.id, to) {
                    trail.push((n.id, to));
                }
            }
        }
        let want: Vec<(usize, usize)> = path.windows(2).map(|w| (w[0], w[1])).collect();
        assert_eq!(
            trail, want,
            "seed {seed}: the walk back is on the trail already"
        );
        // A link between two nodes you've been on but never walked isn't on it.
        for n in &g.net.nodes {
            for &to in &n.links {
                if !want.contains(&(n.id, to)) {
                    assert!(!g.travelled(n.id, to));
                }
            }
        }
    }
}
