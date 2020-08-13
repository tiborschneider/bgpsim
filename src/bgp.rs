//! Module containing definitions for BGP

use crate::{AsId, Prefix, RouterId};

/// Bgo Route
/// The following attributes are omitted
/// - ORIGIN: assumed to be always set to IGP
/// - ATOMIC_AGGREGATE: not used
/// - AGGREGATOR: not used
#[derive(Debug, Clone)]
pub struct BgpRoute {
    pub prefix: Prefix,
    pub as_path: Vec<AsId>,
    pub next_hop: RouterId,
    pub local_pref: Option<u32>,
    pub med: Option<u32>,
}

impl BgpRoute {
    /// Applies the default values for any non-mandatory field
    pub fn apply_default(&mut self) {
        self.local_pref = Some(self.local_pref.unwrap_or(100));
        self.med = Some(self.med.unwrap_or(0));
    }

    /// returns a clone of self, with the default values applied for any non-mandatory field.
    pub fn clone_default(&self) -> Self {
        Self {
            prefix: self.prefix,
            as_path: self.as_path.clone(),
            next_hop: self.next_hop,
            local_pref: Some(self.local_pref.unwrap_or(100)),
            med: Some(self.med.unwrap_or(0)),
        }
    }
}

impl PartialEq for BgpRoute {
    fn eq(&self, other: &Self) -> bool {
        let s = self.clone_default();
        let o = other.clone_default();
        s.prefix == o.prefix
            && s.as_path == other.as_path
            && s.next_hop == o.next_hop
            && s.local_pref == o.local_pref
            && s.med == o.med
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BgpSessionType {
    IBgpPeer,
    IBgpClient,
    EBgp,
}

impl BgpSessionType {
    /// returns true if the session type is EBgp
    pub fn is_ebgp(&self) -> bool {
        match self {
            Self::EBgp => true,
            _ => false,
        }
    }

    /// returns true if the session type is IBgp
    pub fn is_ibgp(&self) -> bool {
        !self.is_ebgp()
    }
}

#[derive(Debug, Clone)]
pub enum BgpEvent {
    Withdraw(Prefix),
    Update(BgpRoute),
}
