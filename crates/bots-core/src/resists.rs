//! Resists in a row of each spell on each mob, so a line a mob keeps
//! resisting (a slow on a slow-immune mob) is given up on that mob.

/// How a cast on a mob ended, as far as resists go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Landed,
    Resisted,
    /// The mob can't have it: given up on at once.
    Immune,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resists {
    /// Spell by the INI's name, mob by spawn ID, resists in a row.
    counts: Vec<(String, u32, u8)>,
}

impl Resists {
    /// Counts a cast's outcome, and returns the resists in a row now
    /// (`u8::MAX` once immune).
    pub fn record(&mut self, spell: &str, mob: u32, outcome: Outcome) -> u8 {
        let index = match self.counts.iter().position(|(name, id, _)| name == spell && *id == mob) {
            Some(index) => index,
            None => {
                self.counts.push((spell.to_owned(), mob, 0));
                self.counts.len() - 1
            }
        };
        let count = &mut self.counts[index].2;
        *count = match outcome {
            Outcome::Landed => 0,
            Outcome::Resisted => count.saturating_add(1),
            Outcome::Immune => u8::MAX,
        };
        *count
    }

    /// Resists in a row of `spell` on `mob`.
    pub fn count(&self, spell: &str, mob: u32) -> u8 {
        self.counts.iter().find(|(name, id, _)| name == spell && *id == mob).map_or(0, |(_, _, count)| *count)
    }

    /// Forgets every mob but `mob`, on engaging it.
    pub fn keep_only(&mut self, mob: u32) {
        self.counts.retain(|(_, id, _)| *id == mob);
    }

    /// Forgets every mob, on `/bots reload`.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_resists_in_a_row_per_spell_and_mob() {
        let mut resists = Resists::default();
        assert_eq!(resists.record("Togor's Insects", 7, Outcome::Resisted), 1);
        assert_eq!(resists.record("Togor's Insects", 7, Outcome::Resisted), 2);
        assert_eq!(resists.record("Malosi", 7, Outcome::Resisted), 1, "another spell");
        assert_eq!(resists.record("Togor's Insects", 8, Outcome::Resisted), 1, "another mob");
        assert_eq!(resists.count("Togor's Insects", 7), 2);
        assert_eq!(resists.record("Togor's Insects", 7, Outcome::Landed), 0, "in a row");
        assert_eq!(resists.record("Malosi", 7, Outcome::Immune), u8::MAX);
        assert_eq!(resists.record("Malosi", 7, Outcome::Resisted), u8::MAX, "immune stays");
    }

    #[test]
    fn forgets_other_mobs() {
        let mut resists = Resists::default();
        resists.record("Togor's Insects", 7, Outcome::Resisted);
        resists.record("Togor's Insects", 8, Outcome::Resisted);
        resists.keep_only(8);
        assert_eq!((resists.count("Togor's Insects", 7), resists.count("Togor's Insects", 8)), (0, 1));
        resists.clear();
        assert_eq!(resists.count("Togor's Insects", 8), 0);
    }
}
