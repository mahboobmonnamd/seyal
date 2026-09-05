use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControlBoundaryPolicy {
    BreakOnAnyControl,
    PreserveNonPositional,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct AppendProbe {
    cluster: String,
    append_allowed: bool,
    joined_scalars: usize,
    restarted_clusters: usize,
}

impl AppendProbe {
    fn print(&mut self, scalar: char) {
        if self.append_allowed && !self.cluster.is_empty() && same_grapheme(&self.cluster, scalar) {
            self.cluster.push(scalar);
            self.joined_scalars += 1;
        } else {
            self.cluster.clear();
            self.cluster.push(scalar);
            self.restarted_clusters += 1;
        }
        self.append_allowed = true;
    }

    fn non_positional_control(&mut self, policy: ControlBoundaryPolicy) {
        if policy == ControlBoundaryPolicy::BreakOnAnyControl {
            self.append_allowed = false;
        }
    }

    fn cursor_moved(&mut self) {
        self.append_allowed = false;
    }
}

fn same_grapheme(active: &str, scalar: char) -> bool {
    let mut candidate = String::with_capacity(active.len() + scalar.len_utf8());
    candidate.push_str(active);
    candidate.push(scalar);
    candidate.graphemes(true).count() == 1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mutation {
    Bell,
    StyleChange(u8),
    HyperlinkStateChange,
    CursorMove { row: usize, col: usize },
    EraseAnchoredCell,
    InsertDeleteCells,
    ScreenSwitch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Anchor {
    row: usize,
    col: usize,
    generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AnchoredCluster {
    text: String,
    anchor: Anchor,
    style: u8,
}

#[derive(Debug)]
struct AnchoredAppendProbe {
    row: usize,
    col: usize,
    mutation_generation: u64,
    current_style: u8,
    active: Option<AnchoredCluster>,
}

impl Default for AnchoredAppendProbe {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            mutation_generation: 0,
            current_style: 0,
            active: None,
        }
    }
}

impl AnchoredAppendProbe {
    fn print(&mut self, scalar: char) {
        let can_append = self.active.as_ref().is_some_and(|active| {
            active.anchor.row == self.row
                && active.anchor.col == self.col
                && active.anchor.generation == self.mutation_generation
                && same_grapheme(&active.text, scalar)
        });

        if can_append {
            self.active
                .as_mut()
                .expect("append candidate was present")
                .text
                .push(scalar);
            return;
        }

        self.active = Some(AnchoredCluster {
            text: scalar.to_string(),
            anchor: Anchor {
                row: self.row,
                col: self.col,
                generation: self.mutation_generation,
            },
            style: self.current_style,
        });
    }

    fn apply(&mut self, mutation: Mutation) {
        match mutation {
            Mutation::Bell | Mutation::HyperlinkStateChange => {
                // These actions change neither the anchored cell nor cursor
                // location. They therefore do not invalidate append identity.
            }
            Mutation::StyleChange(style) => {
                // Style affects the next newly-created grapheme. A combining
                // scalar that continues the currently anchored grapheme does
                // not retroactively restyle the cluster lead.
                self.current_style = style;
            }
            Mutation::CursorMove { row, col } => {
                self.row = row;
                self.col = col;
                self.invalidate_anchor();
            }
            Mutation::EraseAnchoredCell | Mutation::InsertDeleteCells | Mutation::ScreenSwitch => {
                self.invalidate_anchor();
            }
        }
    }

    fn invalidate_anchor(&mut self) {
        self.mutation_generation = self.mutation_generation.wrapping_add(1);
    }

    fn active(&self) -> &AnchoredCluster {
        self.active.as_ref().expect("probe has an active cluster")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum GridCell {
    Empty,
    Lead { text: String, width: u8 },
    Continuation { lead_col: usize },
}

#[derive(Debug)]
struct ProbeRow {
    cells: Vec<GridCell>,
}

impl ProbeRow {
    fn new(columns: usize) -> Self {
        Self {
            cells: vec![GridCell::Empty; columns],
        }
    }

    fn write_cluster(&mut self, col: usize, text: &str) {
        assert!(col < self.cells.len());
        self.erase_cluster_at(col);

        let width = UnicodeWidthStr::width(text);
        assert!(
            (1..=2).contains(&width),
            "grid probe only covers width-1/2 clusters"
        );
        assert!(col + width <= self.cells.len());

        self.cells[col] = GridCell::Lead {
            text: text.to_owned(),
            width: width as u8,
        };
        if width == 2 {
            self.erase_cluster_at(col + 1);
            self.cells[col + 1] = GridCell::Continuation { lead_col: col };
        }
    }

    fn erase_cluster_at(&mut self, col: usize) {
        if col >= self.cells.len() {
            return;
        }
        let lead_col = match &self.cells[col] {
            GridCell::Empty => return,
            GridCell::Lead { .. } => col,
            GridCell::Continuation { lead_col } => *lead_col,
        };
        self.erase_lead(lead_col);
    }

    fn erase_lead(&mut self, lead_col: usize) {
        let width = match self.cells.get(lead_col) {
            Some(GridCell::Lead { width, .. }) => usize::from(*width),
            _ => return,
        };
        let end = lead_col.saturating_add(width).min(self.cells.len());
        for cell in &mut self.cells[lead_col..end] {
            *cell = GridCell::Empty;
        }
    }

    fn occupied_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| !matches!(cell, GridCell::Empty))
            .count()
    }
}

pub(crate) fn report_mutation_semantics() {
    for policy in [
        ControlBoundaryPolicy::BreakOnAnyControl,
        ControlBoundaryPolicy::PreserveNonPositional,
    ] {
        let mut probe = AppendProbe::default();
        probe.print('e');
        probe.non_positional_control(policy);
        probe.print('\u{301}');
        println!(
            "MUTATION\tcontrol-between-combining\tpolicy={policy:?}\tcluster={:?}\tjoined={}\trestarts={}",
            probe.cluster, probe.joined_scalars, probe.restarted_clusters
        );
    }

    let mut anchored = AnchoredAppendProbe::default();
    anchored.print('e');
    anchored.apply(Mutation::Bell);
    anchored.apply(Mutation::StyleChange(7));
    anchored.apply(Mutation::HyperlinkStateChange);
    anchored.print('\u{301}');
    println!(
        "MUTATION\tanchored-non-positional\tcluster={:?}\tcluster_style={}\tcurrent_style={}\tgeneration={}",
        anchored.active().text,
        anchored.active().style,
        anchored.current_style,
        anchored.mutation_generation
    );

    let mut moved = AppendProbe::default();
    moved.print('e');
    moved.cursor_moved();
    moved.print('\u{301}');
    println!(
        "MUTATION\tcursor-move-breaks-append\tcluster={:?}\tjoined={}\trestarts={}",
        moved.cluster, moved.joined_scalars, moved.restarted_clusters
    );

    let mut row = ProbeRow::new(4);
    row.write_cluster(1, "界");
    let before = row.occupied_cells();
    row.erase_cluster_at(2);
    println!(
        "MUTATION\twide-continuation-erase\tbefore={before}\tafter={}",
        row.occupied_cells()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positional_cursor_motion_breaks_cluster_append_eligibility() {
        let mut probe = AppendProbe::default();
        probe.print('e');
        probe.cursor_moved();
        probe.print('\u{301}');
        assert_eq!(probe.joined_scalars, 0);
        assert_eq!(probe.restarted_clusters, 2);
        assert_eq!(probe.cluster, "\u{301}");
    }

    #[test]
    fn non_positional_control_policy_is_observably_architectural() {
        let mut preserve = AppendProbe::default();
        preserve.print('e');
        preserve.non_positional_control(ControlBoundaryPolicy::PreserveNonPositional);
        preserve.print('\u{301}');

        let mut break_all = AppendProbe::default();
        break_all.print('e');
        break_all.non_positional_control(ControlBoundaryPolicy::BreakOnAnyControl);
        break_all.print('\u{301}');

        assert_eq!(preserve.cluster, "e\u{301}");
        assert_eq!(preserve.joined_scalars, 1);
        assert_eq!(break_all.cluster, "\u{301}");
        assert_eq!(break_all.joined_scalars, 0);
    }

    #[test]
    fn non_positional_actions_preserve_anchor_identity() {
        let mut probe = AnchoredAppendProbe::default();
        probe.print('e');
        let generation = probe.mutation_generation;
        probe.apply(Mutation::Bell);
        probe.apply(Mutation::HyperlinkStateChange);
        probe.print('\u{301}');
        assert_eq!(probe.active().text, "e\u{301}");
        assert_eq!(probe.mutation_generation, generation);
    }

    #[test]
    fn style_change_does_not_retroactively_split_or_restyle_active_grapheme() {
        let mut probe = AnchoredAppendProbe::default();
        probe.current_style = 3;
        probe.print('e');
        probe.apply(Mutation::StyleChange(9));
        probe.print('\u{301}');
        assert_eq!(probe.active().text, "e\u{301}");
        assert_eq!(probe.active().style, 3);
        assert_eq!(probe.current_style, 9);
    }

    #[test]
    fn destructive_mutations_invalidate_active_anchor() {
        for mutation in [
            Mutation::EraseAnchoredCell,
            Mutation::InsertDeleteCells,
            Mutation::ScreenSwitch,
        ] {
            let mut probe = AnchoredAppendProbe::default();
            probe.print('e');
            probe.apply(mutation);
            probe.print('\u{301}');
            assert_eq!(probe.active().text, "\u{301}");
        }
    }

    #[test]
    fn cursor_move_invalidates_active_anchor_even_if_cursor_returns() {
        let mut probe = AnchoredAppendProbe::default();
        probe.print('e');
        probe.apply(Mutation::CursorMove { row: 0, col: 1 });
        probe.apply(Mutation::CursorMove { row: 0, col: 0 });
        probe.print('\u{301}');
        assert_eq!(probe.active().text, "\u{301}");
    }

    #[test]
    fn overwriting_wide_lead_erases_its_continuation() {
        let mut row = ProbeRow::new(4);
        row.write_cluster(1, "界");
        assert_eq!(row.occupied_cells(), 2);
        row.write_cluster(1, "A");
        assert_eq!(row.occupied_cells(), 1);
        assert!(matches!(&row.cells[2], GridCell::Empty));
    }

    #[test]
    fn overwriting_wide_continuation_erases_the_full_previous_cluster() {
        let mut row = ProbeRow::new(4);
        row.write_cluster(1, "界");
        assert_eq!(row.occupied_cells(), 2);
        row.write_cluster(2, "B");
        assert_eq!(row.occupied_cells(), 1);
        assert!(matches!(&row.cells[1], GridCell::Empty));
        assert!(matches!(&row.cells[2], GridCell::Lead { text, width: 1 } if text == "B"));
    }
}
