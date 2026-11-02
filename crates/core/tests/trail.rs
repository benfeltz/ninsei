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
        let mut path = vec![g.at];
        for _ in 0..3 {
            let Some(&to) = g.reachable().last() else {
                break;
            };
            g.apply(Action::Move(to));
            path.push(to);
        }
        let mut trail = Vec::new();
        for n in &g.net.nodes {
            for &to in &n.links {
                if g.travelled(n.id, to) {
                    trail.push((n.id, to));
                }
            }
        }
        let want: Vec<(usize, usize)> = path.windows(2).map(|w| (w[0], w[1])).collect();
        assert_eq!(trail, want, "seed {seed}");
    }
}
