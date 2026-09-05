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
