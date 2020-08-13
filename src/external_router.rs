use crate::bgp::{BgpEvent, BgpRoute};
use crate::event::{Event, EventQueue};
use crate::{AsId, DeviceError, NetworkDevice, Prefix, RouterId};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ExternalRouter {
    name: &'static str,
    router_id: RouterId,
    as_id: AsId,
    pub neighbors: HashSet<RouterId>,
}

impl NetworkDevice for ExternalRouter {
    /// Create a new NetworkDevice instance
    fn new(name: &'static str, router_id: RouterId, as_id: AsId) -> Self {
        Self {
            name,
            router_id,
            as_id,
            neighbors: HashSet::new(),
        }
    }

    /// Handle an `Event` and produce the necessary result
    fn handle_event(&mut self, _event: Event, _queue: &mut EventQueue) -> Result<(), DeviceError> {
        Ok(())
    }

    /// Return the ID of the network device
    fn router_id(&self) -> RouterId {
        self.router_id
    }

    /// return the AS of the network device
    fn as_id(&self) -> AsId {
        self.as_id
    }

    /// Return the name of the network device
    fn name(&self) -> &'static str {
        self.name
    }
}

impl ExternalRouter {
    /// Send an BGP UPDATE to all neighbors with the new route
    pub fn advertise_prefix(
        &self,
        prefix: Prefix,
        as_path: Vec<AsId>,
        med: Option<u32>,
        queue: &mut EventQueue,
    ) {
        let route = BgpRoute {
            prefix,
            as_path,
            next_hop: self.router_id,
            local_pref: None,
            med,
        };
        let bgp_event = BgpEvent::Update(route);
        for neighbor in self.neighbors.iter() {
            queue.push_back(Event::Bgp(self.router_id, *neighbor, bgp_event.clone()));
        }
    }

    /// Send a BGP WITHDRAW to all neighbors for the given prefix
    pub fn widthdraw_prefix(&self, prefix: Prefix, queue: &mut EventQueue) {
        for neighbor in self.neighbors.iter() {
            queue.push_back(Event::Bgp(
                self.router_id,
                *neighbor,
                BgpEvent::Withdraw(prefix),
            ));
        }
    }
}
