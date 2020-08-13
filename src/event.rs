//! Module for defining events

use crate::bgp::BgpEvent;
use crate::RouterId;
use std::collections::VecDeque;

/// Event to handle
#[derive(Debug, Clone)]
pub enum Event {
    /// BGP Event from `#0` to `#1`
    Bgp(RouterId, RouterId, BgpEvent),
}

/// Event queue for enqueuing events.
pub type EventQueue = VecDeque<Event>;
