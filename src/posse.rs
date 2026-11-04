//! Reading other characters' status from the posse this one is in.

use std::time::{Duration, Instant};

use mq_emu::{Ini, Mq};
use posse_core::{DEFAULT_SECTION, Roster, SETTINGS_FILE, Status, auto_join_posse, mapping_name, settings_section};
use posse_shm::{Mapping, Processes, now_ms};

use crate::Bots;

/// How often the list of members is rebuilt; their status is read fresh.
const ROSTER_REFRESH: Duration = Duration::from_secs(1);

pub struct PosseLink {
    pub name: String,
    mapping: Mapping,
    roster: Roster,
    processes: Processes,
    refreshed: Option<Instant>,
}

impl PosseLink {
    /// The posse MQ2Posse joins this character to, from its settings.
    /// `None` with `AutoJoin=0`.
    pub fn open(mq: &Mq<Bots>, character: &str) -> Result<Option<PosseLink>, String> {
        let server = mq.server_name().unwrap_or_default();
        let section = settings_section(&server, character);
        let ini = Ini::new(mq.config_dir().join(SETTINGS_FILE)).ok();
        let read = |key: &str| ini.as_ref().and_then(|ini| ini.get(&section, key).or_else(|| ini.get(DEFAULT_SECTION, key)));
        let Some(name) = auto_join_posse(read) else { return Ok(None) };
        let mapping = Mapping::open(&mapping_name(&server, &name)).map_err(|e| format!("couldn't open posse {name}: {e}"))?;
        Ok(Some(PosseLink { name, mapping, roster: Roster::default(), processes: Processes::default(), refreshed: None }))
    }

    /// A member's status, unless it's stale (its client stopped publishing).
    pub fn status(&mut self, name: &str) -> Option<Status> {
        let region = self.mapping.region();
        if self.refreshed.is_none_or(|at| at.elapsed() >= ROSTER_REFRESH) {
            self.roster.refresh(region, &mut self.processes);
            self.refreshed = Some(Instant::now());
        }
        self.roster.read(region, name).filter(|status| !status.is_stale(now_ms()))
    }
}
