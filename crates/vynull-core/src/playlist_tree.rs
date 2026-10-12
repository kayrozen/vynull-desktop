//! The playlist sidebar as a tree: folders that can be closed, and which rows that leaves visible.

use std::collections::{BTreeSet, HashSet};

use vynull_api::PlaylistInfo;

/// Playlists in tree order with their depth. Orphans (a missing parent) show at the top level.
pub fn flatten(pls: &[PlaylistInfo]) -> Vec<(PlaylistInfo, usize)> {
    let ids: HashSet<i64> = pls.iter().map(|p| p.id).collect();
    let mut out = Vec::new();
    fn walk(
        pls: &[PlaylistInfo],
        ids: &HashSet<i64>,
        parent: i64,
        depth: usize,
        out: &mut Vec<(PlaylistInfo, usize)>,
    ) {
        let mut kids: Vec<&PlaylistInfo> = pls
            .iter()
            .filter(|p| {
                if parent == 0 {
                    p.parent_id == 0 || !ids.contains(&p.parent_id)
                } else {
                    p.parent_id == parent
                }
            })
            .collect();
        kids.sort_by_key(|p| (p.sort_order, p.id));
        for p in kids {
            out.push((p.clone(), depth));
            if p.is_folder && depth < 8 {
                walk(pls, ids, p.id, depth + 1, out);
            }
        }
    }
    walk(pls, &ids, 0, 0, &mut out);
    out
}

/// The rows left after hiding what is inside the folders in `closed`. A closed folder itself stays.
pub fn visible(
    flat: &[(PlaylistInfo, usize)],
    closed: &BTreeSet<i64>,
) -> Vec<(PlaylistInfo, usize)> {
    let mut out = Vec::new();
    // Depth of the closed folder whose contents are being skipped.
    let mut skipping: Option<usize> = None;
    for (p, depth) in flat {
        if let Some(d) = skipping {
            if *depth > d {
                continue;
            }
            skipping = None;
        }
        out.push((p.clone(), *depth));
        if p.is_folder && closed.contains(&p.id) {
            skipping = Some(*depth);
        }
    }
    out
}

/// The folders above playlist `id`, nearest first. Stops if the tree loops.
pub fn ancestors(pls: &[PlaylistInfo], id: i64) -> Vec<i64> {
    let mut out = Vec::new();
    let mut at = id;
    while let Some(p) = pls.iter().find(|p| p.id == at) {
        let parent = p.parent_id;
        if parent == 0 || parent == id || out.contains(&parent) {
            break;
        }
        out.push(parent);
        at = parent;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pl(id: i64, parent: i64, folder: bool, order: i64) -> PlaylistInfo {
        PlaylistInfo {
            id,
            parent_id: parent,
            is_folder: folder,
            sort_order: order,
            name: format!("p{id}"),
            ..Default::default()
        }
    }

    /// 1 Archive/ (2 Summer/ (4 a, 5 b), 3 c), 6 Warm-up, 7 Sets/ (empty)
    fn tree() -> Vec<PlaylistInfo> {
        vec![
            pl(1, 0, true, 0),
            pl(2, 1, true, 0),
            pl(3, 1, false, 1),
            pl(4, 2, false, 0),
            pl(5, 2, false, 1),
            pl(6, 0, false, 1),
            pl(7, 0, true, 2),
        ]
    }

    fn ids(v: &[(PlaylistInfo, usize)]) -> Vec<i64> {
        v.iter().map(|(p, _)| p.id).collect()
    }

    #[test]
    fn flatten_is_tree_order_with_depths() {
        let f = flatten(&tree());
        assert_eq!(ids(&f), vec![1, 2, 4, 5, 3, 6, 7]);
        assert_eq!(
            f.iter().map(|(_, d)| *d).collect::<Vec<_>>(),
            vec![0, 1, 2, 2, 1, 0, 0]
        );
    }

    #[test]
    fn closing_a_folder_hides_everything_inside_it_but_not_after_it() {
        let f = flatten(&tree());
        let closed = |ids: &[i64]| ids.iter().copied().collect::<BTreeSet<i64>>();
        assert_eq!(
            ids(&visible(&f, &closed(&[]))),
            vec![1, 2, 4, 5, 3, 6, 7],
            "all open"
        );
        assert_eq!(
            ids(&visible(&f, &closed(&[1]))),
            vec![1, 6, 7],
            "the outer folder takes the inner one with it"
        );
        assert_eq!(
            ids(&visible(&f, &closed(&[2]))),
            vec![1, 2, 3, 6, 7],
            "an inner folder hides only its own"
        );
        assert_eq!(ids(&visible(&f, &closed(&[1, 2]))), vec![1, 6, 7]);
        assert_eq!(
            ids(&visible(&f, &closed(&[7]))),
            vec![1, 2, 4, 5, 3, 6, 7],
            "an empty folder hides nothing"
        );
        assert_eq!(
            ids(&visible(&f, &closed(&[6]))),
            vec![1, 2, 4, 5, 3, 6, 7],
            "a playlist is not a folder"
        );
        assert_eq!(
            ids(&visible(&f, &closed(&[99]))),
            ids(&f),
            "an unknown id is ignored"
        );
    }

    #[test]
    fn ancestors_list_the_folders_above() {
        let t = tree();
        assert_eq!(ancestors(&t, 4), vec![2, 1]);
        assert_eq!(ancestors(&t, 3), vec![1]);
        assert_eq!(ancestors(&t, 6), Vec::<i64>::new());
        assert_eq!(ancestors(&t, 42), Vec::<i64>::new(), "unknown");
    }

    #[test]
    fn a_looping_tree_does_not_hang() {
        let t = vec![pl(1, 2, true, 0), pl(2, 1, true, 0), pl(3, 3, false, 0)];
        assert!(ancestors(&t, 1).len() <= 2);
        assert!(ancestors(&t, 3).len() <= 1);
    }
}
