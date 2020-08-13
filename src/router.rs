//! Module defining an internal router with BGP functionality.

use crate::bgp::{BgpEvent, BgpRoute, BgpSessionType};
use crate::{AsId, DeviceError, IgpNetwork, LinkWeight, NetworkDevice, Prefix, RouterId};
use crate::{Event, EventQueue};
use petgraph::algo::{bellman_ford, FloatMeasure};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct Router {
    /// Name of the router
    name: &'static str,
    /// ID of the router
    router_id: RouterId,
    /// AS Id of the router
    as_id: AsId,
    /// forwarding table for IGP messages
    pub igp_forwarding_table: HashMap<RouterId, Option<(RouterId, LinkWeight)>>,
    /// Open iBGP connections to peers or other route reflectors
    ibgp_peer_sessions: HashSet<RouterId>,
    /// Open iBGP connections to clients
    ibgp_client_sessions: HashSet<RouterId>,
    /// Open eBGP connections
    ebgp_sessions: HashSet<RouterId>,
    /// Table containing all received entries. It is represented as a hashmap, mapping the prefixes
    /// to another hashmap, which maps the received router id to the entry. This way, we can store
    /// one entry for every prefix and every session.
    bgp_rib_in: HashMap<Prefix, HashMap<RouterId, RIBEntry>>,
    /// Table containing all selected best routes. It is represented as a hashmap, mapping the
    /// prefixes to the table entry
    bgp_rib: HashMap<Prefix, RIBEntry>,
    /// Table containing all exported routes, represented as a hashmap mapping the neighboring
    /// RouterId (of a BGP session) to the table entries.
    bgp_rib_out: HashMap<Prefix, HashMap<RouterId, RIBEntry>>,
    /// Set of known bgp prefixes
    bgp_known_prefixes: HashSet<Prefix>,
    /// BGP configuration for tagging the local_pref of routes announced via eBGP, based on the
    /// router from which the route originates.
    pub policy_bgp_local_pref: HashMap<RouterId, u32>,
    /// BGP configuration for when to export routes to an eBGP peer, based on the next hop field of
    /// the route to be exported. This way, business relationships can be implemented, by
    /// prohibiting routes from a provider to be exported to a different provider.
    /// The tuple tells that a route, advertised by #0 should *not* be exported to the peer #1
    pub policy_bgp_route_no_export: HashSet<(RouterId, RouterId)>,
}

impl NetworkDevice for Router {
    fn new(name: &'static str, router_id: RouterId, as_id: AsId) -> Router {
        Router {
            name,
            router_id,
            as_id,
            igp_forwarding_table: HashMap::new(),
            ibgp_peer_sessions: HashSet::new(),
            ibgp_client_sessions: HashSet::new(),
            ebgp_sessions: HashSet::new(),
            bgp_rib_in: HashMap::new(),
            bgp_rib: HashMap::new(),
            bgp_rib_out: HashMap::new(),
            bgp_known_prefixes: HashSet::new(),
            policy_bgp_local_pref: HashMap::new(),
            policy_bgp_route_no_export: HashSet::new(),
        }
    }

    /// Return the idx of the Router
    fn router_id(&self) -> RouterId {
        self.router_id
    }

    /// Return the name of the Router
    fn name(&self) -> &'static str {
        self.name
    }

    /// return the AS ID of the Router
    fn as_id(&self) -> AsId {
        self.as_id
    }

    /// handle an `Event`, and enqueue several resulting events
    fn handle_event(&mut self, event: Event, queue: &mut EventQueue) -> Result<(), DeviceError> {
        match event {
            Event::Bgp(from, to, bgp_event) if to == self.router_id => {
                // phase 1 of BGP protocol
                let prefix = match bgp_event {
                    BgpEvent::Update(route) => self.insert_bgp_route(route, from)?,
                    BgpEvent::Withdraw(prefix) => self.remove_bgp_route(prefix, from),
                };
                self.bgp_known_prefixes.insert(prefix);
                // phase 2
                self.run_bgp_decision_process_for_prefix(prefix)?;
                // phase 3
                self.run_bgp_route_dissemination_for_prefix(prefix, queue)
            }
            _ => Ok(()),
        }
    }
}

impl Router {
    /// establish a bgp session with a peer
    /// `session_type` tells that `target` is in relation to `self`. If `session_type` is
    /// `BgpSessionType::IbgpClient`, then the `target` is added as client to `self`.
    pub fn establish_bgp_session(
        &mut self,
        target: RouterId,
        session_type: BgpSessionType,
    ) -> Result<(), DeviceError> {
        if self.ebgp_sessions.contains(&target)
            || self.ibgp_peer_sessions.contains(&target)
            || self.ibgp_client_sessions.contains(&target)
        {
            return Err(DeviceError::SessionAlreadyExists(target));
        }

        match session_type {
            BgpSessionType::EBgp => self.ebgp_sessions.insert(target),
            BgpSessionType::IBgpPeer => self.ibgp_peer_sessions.insert(target),
            BgpSessionType::IBgpClient => self.ibgp_client_sessions.insert(target),
        };

        Ok(())
    }

    /// remove a bgp session
    pub fn close_bgp_session(&mut self, target: RouterId) -> Result<(), DeviceError> {
        let mut removed: bool = false;
        if self.ebgp_sessions.remove(&target) {
            removed = true;
        }
        if self.ibgp_peer_sessions.remove(&target) {
            removed = true;
        }
        if self.ibgp_client_sessions.remove(&target) {
            removed = true;
        }
        if !removed {
            return Err(DeviceError::NoBgpSession(target));
        }
        for prefix in self.bgp_known_prefixes.clone() {
            self.bgp_rib_in
                .get_mut(&prefix)
                .and_then(|rib| rib.remove(&target));
            self.bgp_rib_out
                .get_mut(&prefix)
                .and_then(|rib| rib.remove(&target));
        }
        Ok(())
    }

    /// write forawrding table based on graph
    /// This function requres that all RouterIds are set to the GraphId.
    pub fn write_igp_forwarding_table(&mut self, graph: &IgpNetwork) -> Result<(), DeviceError> {
        // clear the forwarding table
        self.igp_forwarding_table = HashMap::new();
        // compute shortest path to all other nodes in the graph
        let (path_weights, predecessors) = bellman_ford(graph, self.router_id.into()).unwrap();
        let mut paths: Vec<(RouterId, LinkWeight, Option<RouterId>)> = path_weights
            .into_iter()
            .zip(predecessors.into_iter())
            .enumerate()
            .map(|(i, (w, p))| ((i as u32).into(), w, p.map(|x| x)))
            .collect();
        paths.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        for (router, cost, predecessor) in paths {
            if cost == LinkWeight::infinite() {
                self.igp_forwarding_table.insert(router, None);
                continue;
            }
            let next_hop = if let Some(predecessor) = predecessor {
                // the predecessor must already be inserted into the forwarding table, because we sorted the table
                if predecessor == self.router_id {
                    router
                } else {
                    self.igp_forwarding_table
                        .get(&predecessor)
                        .unwrap() // first unwrap for get, which returns an option
                        .unwrap() // second unwrap to unwrap wether the route exists (it must!)
                        .0
                }
            } else {
                router
            };
            self.igp_forwarding_table
                .insert(router, Some((next_hop, cost)));
        }
        Ok(())
    }

    /// Run the bgp decision process, select the best route. This does not execute route
    /// dissemination!
    pub fn bgp_decision_process(&mut self) -> Result<(), DeviceError> {
        for prefix in self.bgp_known_prefixes.clone() {
            self.run_bgp_decision_process_for_prefix(prefix)?
        }
        Ok(())
    }

    /// Execute route dissemination, e.g. send all necessary updates to all peers
    pub fn bgp_route_dissemination(&mut self, queue: &mut EventQueue) -> Result<(), DeviceError> {
        for prefix in self.bgp_known_prefixes.clone() {
            self.run_bgp_route_dissemination_for_prefix(prefix, queue)?
        }
        Ok(())
    }

    /// get the IGP next hop for a prefix
    pub fn get_next_hop(&self, prefix: Prefix) -> Option<RouterId> {
        match self.bgp_rib.get(&prefix) {
            Some(entry) => self
                .igp_forwarding_table
                .get(&entry.route.next_hop)
                .unwrap()
                .map(|e| e.0),
            None => None,
        }
    }

    /// Return a list of all known bgp routes for a given origin
    pub fn get_known_bgp_routes(&self, prefix: Prefix) -> Result<Vec<RIBEntry>, DeviceError> {
        let mut entries: Vec<RIBEntry> = Vec::new();
        if let Some(table) = self.bgp_rib_in.get(&prefix) {
            for e in table.values() {
                entries.push(self.process_bgp_rib_in_route(e)?);
            }
        }
        Ok(entries)
    }

    /// Returns the selected bgp route for the prefix, or returns None
    pub fn get_selected_bgp_route(&self, prefix: Prefix) -> Option<RIBEntry> {
        self.bgp_rib.get(&prefix).map(|r| r.clone())
    }

    // -----------------
    // Private Functions
    // -----------------

    /// only run bgp decision process (phase 2)
    fn run_bgp_decision_process_for_prefix(&mut self, prefix: Prefix) -> Result<(), DeviceError> {
        // search the best route and compare
        let old_entry = self.bgp_rib.get(&prefix);
        let mut new_entry = None;

        // find the new best route
        if let Some(rib_in) = self.bgp_rib_in.get(&prefix) {
            for entry_unprocessed in rib_in.values() {
                let entry = self.process_bgp_rib_in_route(entry_unprocessed)?;
                let mut better = true;
                if let Some(current_best) = new_entry.as_ref() {
                    better = &entry > current_best;
                }
                if better {
                    new_entry = Some(entry)
                }
            }
        }

        // check if the entry will get changed
        if new_entry.as_ref() != old_entry {
            // replace the entry
            if let Some(new_entry) = new_entry {
                // insert the new entry
                self.bgp_rib.insert(prefix, new_entry);
            } else {
                self.bgp_rib.remove(&prefix);
            }
        }
        Ok(())
    }

    /// only run bgp route dissemination (phase 3)
    fn run_bgp_route_dissemination_for_prefix(
        &mut self,
        prefix: Prefix,
        queue: &mut EventQueue,
    ) -> Result<(), DeviceError> {
        if !self.bgp_rib_out.contains_key(&prefix) {
            self.bgp_rib_out.insert(prefix, HashMap::new());
        }

        let bgp_peers: HashSet<RouterId> = self
            .ibgp_client_sessions
            .union(&self.ibgp_peer_sessions)
            .cloned()
            .collect::<HashSet<_>>()
            .union(&self.ebgp_sessions)
            .cloned()
            .collect::<HashSet<_>>();

        for peer in bgp_peers {
            // apply the route for the specific peer
            let best_route: Option<RIBEntry> = self
                .bgp_rib
                .get(&prefix)
                .map(|e| self.process_bgp_rib_out_route(e, peer))
                .transpose()?;
            // check if the current information is the same
            let current_route: Option<RIBEntry> = self
                .bgp_rib_out
                .get_mut(&prefix)
                .and_then(|rib| rib.get(&peer).cloned());
            let event = match (best_route, current_route) {
                (Some(best_r), Some(current_r)) if best_r == current_r => {
                    // Nothing to do, no new route received
                    None
                }
                (Some(best_r), Some(_)) => {
                    // Route information was changed
                    if self.should_export_route(best_r.from_id, peer)? {
                        // update the route
                        let event = BgpEvent::Update(best_r.route.clone());
                        self.bgp_rib_out
                            .get_mut(&prefix)
                            .and_then(|rib| rib.insert(peer, best_r));
                        Some(event)
                    } else {
                        // send a withdraw of the old route
                        self.bgp_rib_out
                            .get_mut(&prefix)
                            .and_then(|rib| rib.remove(&peer));
                        Some(BgpEvent::Withdraw(prefix))
                    }
                }
                (Some(best_r), None) => {
                    // New route information received
                    if self.should_export_route(best_r.from_id, peer)? {
                        // send the route
                        let event = BgpEvent::Update(best_r.route.clone());
                        self.bgp_rib_out
                            .get_mut(&prefix)
                            .and_then(|rib| rib.insert(peer, best_r));
                        Some(event)
                    } else {
                        None
                    }
                }
                (None, Some(_)) => {
                    // Current route must be WITHDRAWN, since we do no longer know any route
                    self.bgp_rib_out
                        .get_mut(&prefix)
                        .and_then(|rib| rib.remove(&peer));
                    Some(BgpEvent::Withdraw(prefix))
                }
                (None, None) => {
                    // Nothing to do
                    None
                }
            };
            // add the event to the queue
            if let Some(event) = event {
                queue.push_back(Event::Bgp(self.router_id, peer, event));
            }
        }

        Ok(())
    }

    /// Tries to insert the route into the bgp_rib_in table. If the same route already exists in the table,
    /// replace the route. It returns the prefix for which the route was inserted
    fn insert_bgp_route(&mut self, route: BgpRoute, from: RouterId) -> Result<Prefix, DeviceError> {
        let prefix = route.prefix;
        let from_type = self.get_bgp_session_type(from)?;

        // the incoming bgp routes should not be processed here!
        // This is because when configuration chagnes, the routes should also change without needing
        // to receive them again.
        // Also, we don't yet compute the igp cost.
        let new_entry = RIBEntry {
            route,
            from_type,
            from_id: from,
            igp_cost: None,
        };

        let rib_in = if self.bgp_rib_in.contains_key(&new_entry.route.prefix) {
            self.bgp_rib_in.get_mut(&new_entry.route.prefix).unwrap()
        } else {
            self.bgp_rib_in
                .insert(new_entry.route.prefix, HashMap::new());
            self.bgp_rib_in.get_mut(&new_entry.route.prefix).unwrap()
        };

        // insert the new route. If an old route was received, just ignore that one and drop it.
        rib_in.insert(from, new_entry);

        Ok(prefix)
    }

    /// remove an existing bgp route in bgp_rib_in and returns the prefix for which the route was
    /// inserted.
    fn remove_bgp_route(&mut self, prefix: Prefix, from: RouterId) -> Prefix {
        // check if the prefix does exist in the table
        self.bgp_rib_in
            .get_mut(&prefix)
            .and_then(|rib| rib.remove(&from));
        prefix
    }

    /// process incoming routes from bgp_rib_in
    fn process_bgp_rib_in_route(&self, entry: &RIBEntry) -> Result<RIBEntry, DeviceError> {
        let local_pref = if entry.from_type.is_ebgp() {
            Some(
                self.policy_bgp_local_pref
                    .get(&entry.from_id)
                    .map(|x| *x) // copy the value received from the hashmap
                    .unwrap_or(100), // if no value was received, use default of 100
            )
        } else {
            entry.route.local_pref
        };

        // compute the igp cost
        let igp_cost = if entry.from_type.is_ibgp() {
            self.igp_forwarding_table
                .get(&entry.route.next_hop)
                .ok_or(DeviceError::RouterNotFound(entry.route.next_hop))?
                .ok_or(DeviceError::RouterNotReachable(entry.route.next_hop))?
                .1
        } else {
            0.0
        };

        let mut new_route = entry.route.clone_default();
        new_route.local_pref = local_pref;

        // set the next hop to the egress from router if the message came from externally
        if entry.from_type.is_ebgp() {
            new_route.next_hop = entry.from_id;
        }

        Ok(RIBEntry {
            route: new_route,
            from_type: entry.from_type,
            from_id: entry.from_id,
            igp_cost: Some(igp_cost),
        })
    }

    /// Process a route from bgp_rib for sending it to bgp peers, and storing it into bgp_rib_out.
    /// The entry is cloned and modified
    fn process_bgp_rib_out_route(
        &self,
        entry: &RIBEntry,
        target_peer: RouterId,
    ) -> Result<RIBEntry, DeviceError> {
        let mut new_route = entry.route.clone();
        if self.ebgp_sessions.contains(&target_peer) {
            new_route.next_hop = self.router_id;
            new_route.local_pref = None;
        }
        Ok(RIBEntry {
            route: new_route,
            from_type: self.get_bgp_session_type(target_peer)?,
            from_id: entry.from_id,
            igp_cost: entry.igp_cost,
        })
    }

    /// returns the BgpSessionType for a peer
    fn get_bgp_session_type(&self, peer: RouterId) -> Result<BgpSessionType, DeviceError> {
        if self.ibgp_peer_sessions.contains(&peer) {
            Ok(BgpSessionType::IBgpPeer)
        } else if self.ibgp_client_sessions.contains(&peer) {
            Ok(BgpSessionType::IBgpClient)
        } else if self.ebgp_sessions.contains(&peer) {
            Ok(BgpSessionType::EBgp)
        } else {
            Err(DeviceError::NoBgpSession(peer))
        }
    }

    /// returns a bool which tells to export the route to the target, which was advertised by the
    /// source.
    fn should_export_route(&self, from: RouterId, to: RouterId) -> Result<bool, DeviceError> {
        // never advertise a route to the receiver
        if from == to {
            return Ok(false);
        }
        // read the policy
        if self.policy_bgp_route_no_export.contains(&(from, to)) {
            return Ok(false);
        }
        // check the types
        let from_type = self.get_bgp_session_type(from)?;
        let to_type = self.get_bgp_session_type(to)?;

        Ok(match (from_type, to_type) {
            (BgpSessionType::EBgp, _) => true,
            (BgpSessionType::IBgpClient, _) => true,
            (_, BgpSessionType::EBgp) => true,
            (_, BgpSessionType::IBgpClient) => true,
            _ => false,
        })
    }
}

/// BGP RIB Table entry
#[derive(Debug, Clone)]
pub struct RIBEntry {
    /// the actual bgp route
    pub route: BgpRoute,
    /// the type of session, from which the route was learned
    pub from_type: BgpSessionType,
    /// the client from which the route was learned
    pub from_id: RouterId,
    /// the igp cost to the next_hop
    pub igp_cost: Option<LinkWeight>,
}

impl PartialEq for RIBEntry {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route && self.from_id == other.from_id
    }
}

impl PartialOrd for RIBEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let s = self.route.clone_default();
        let o = other.route.clone_default();

        if s.local_pref > o.local_pref {
            return Some(Ordering::Greater);
        } else if s.local_pref < o.local_pref {
            return Some(Ordering::Less);
        }

        if s.as_path.len() < o.as_path.len() {
            return Some(Ordering::Greater);
        } else if s.as_path.len() > o.as_path.len() {
            return Some(Ordering::Less);
        }

        if s.med < o.med {
            return Some(Ordering::Greater);
        } else if s.med > o.med {
            return Some(Ordering::Less);
        }

        if self.from_type.is_ebgp() && other.from_type.is_ibgp() {
            return Some(Ordering::Greater);
        } else if self.from_type.is_ibgp() && self.from_type.is_ebgp() {
            return Some(Ordering::Less);
        }

        if self.igp_cost.unwrap() < other.igp_cost.unwrap() {
            return Some(Ordering::Greater);
        } else if self.igp_cost > other.igp_cost {
            return Some(Ordering::Less);
        }

        if s.next_hop < o.next_hop {
            return Some(Ordering::Greater);
        } else if s.next_hop > o.next_hop {
            return Some(Ordering::Less);
        }

        if self.from_id < other.from_id {
            return Some(Ordering::Greater);
        } else if self.from_id > other.from_id {
            return Some(Ordering::Less);
        }

        Some(Ordering::Equal)
    }
}
