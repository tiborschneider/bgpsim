use crate::bgp::{BgpEvent, BgpSessionType};
use crate::event::{Event, EventQueue};
use crate::external_router::ExternalRouter;
use crate::router::{RIBEntry, Router};
use crate::{
    AsId, DeviceError, IgpNetwork, LinkWeight, NetworkDevice, NetworkError, Prefix, RouterId,
};
use std::collections::{HashMap, HashSet};

static DEFAULT_STOP_AFTER: usize = 10_000;

#[derive(Debug)]
pub struct Network {
    net: IgpNetwork,
    routers: HashMap<RouterId, Router>,
    external_routers: HashMap<RouterId, ExternalRouter>,
    queue: EventQueue,
    stop_after: Option<usize>,
}

impl Network {
    pub fn new() -> Self {
        Self {
            net: IgpNetwork::new(),
            routers: HashMap::new(),
            external_routers: HashMap::new(),
            queue: EventQueue::new(),
            stop_after: Some(DEFAULT_STOP_AFTER),
        }
    }

    /// Configure the topology to pause the queue and return after a certain number of queue have
    /// been executed. The job queue will remain active. If set to None, the queue will continue
    /// running until converged.
    pub fn stop_after_queue(&mut self, stop_after: Option<usize>) {
        self.stop_after = stop_after;
    }

    /// add a new router to the topology and return
    /// Own as is always set to 65001
    pub fn add_router(&mut self, name: &'static str) -> RouterId {
        let new_router = Router::new(name, self.net.add_node(()), AsId(65001));
        let router_id = new_router.router_id();
        self.routers.insert(router_id, new_router);
        router_id
    }

    /// add a new external router to the topology and return
    pub fn add_external_router(&mut self, name: &'static str, as_id: AsId) -> RouterId {
        let new_router = ExternalRouter::new(name, self.net.add_node(()), as_id);
        let router_id = new_router.router_id();
        self.external_routers.insert(router_id, new_router);
        router_id
    }

    /// # Create an edge
    ///
    /// create an edge between two routers. If `rev_w` is `None`, then the link is treated as
    /// symmetric. Else, the reverse path will have weight `rev_w`. Source and Target may be
    /// external routers. For external routers, an eBGP connection is created.
    pub fn add_edge(
        &mut self,
        source: RouterId,
        target: RouterId,
        weight: LinkWeight,
        rev_w: Option<LinkWeight>,
    ) -> Result<(), NetworkError> {
        // add forward link
        self.net.add_edge(source, target, weight);
        self.net.add_edge(target, source, rev_w.unwrap_or(weight));
        // if source or target is an external router, add them to adjacency list and add ebgp
        // connection
        let source_external = self.external_routers.contains_key(&source);
        let target_external = self.external_routers.contains_key(&target);
        if source_external {
            // add connection from external source to (potentially extern) target
            self.external_routers
                .get_mut(&source)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .neighbors
                .insert(target);
            if !target_external {
                // add eBGP session from intern target to extern source
                self.routers
                    .get_mut(&target)
                    .ok_or(NetworkError::DeviceNotFound(target))?
                    .establish_bgp_session(source, BgpSessionType::EBgp)?;
            }
        }
        if target_external {
            // add connection from external source to (potentially extern) target
            self.external_routers
                .get_mut(&target)
                .ok_or(NetworkError::DeviceNotFound(target))?
                .neighbors
                .insert(source);
            if !source_external {
                // add eBGP session from intern target to extern source
                self.routers
                    .get_mut(&source)
                    .ok_or(NetworkError::DeviceNotFound(source))?
                    .establish_bgp_session(target, BgpSessionType::EBgp)?;
            }
        }
        Ok(())
    }

    /// update the weight of an edge
    pub fn update_edge_weight(
        &mut self,
        source: RouterId,
        target: RouterId,
        weight: LinkWeight,
        rev_w: Option<LinkWeight>,
    ) {
        self.net.update_edge(source, target, weight);
        self.net
            .update_edge(target, source, rev_w.unwrap_or(weight));
    }

    /// # Add an iBGP session
    ///
    /// Adds an iBGP session between source and target. If `route_reflector` is set to false, then
    /// the connection is configured as regular peers. If route reflector is set to true, then the
    /// source is considered as Route Reflector, and the target is considered as client.
    pub fn add_ibgp_session(
        &mut self,
        source: RouterId,
        target: RouterId,
        route_reflector: bool,
        update: bool,
    ) -> Result<bool, NetworkError> {
        if route_reflector {
            self.routers
                .get_mut(&source)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .establish_bgp_session(target, BgpSessionType::IBgpClient)?;
            self.routers
                .get_mut(&target)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .establish_bgp_session(source, BgpSessionType::IBgpPeer)?;
        } else {
            self.routers
                .get_mut(&source)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .establish_bgp_session(target, BgpSessionType::IBgpPeer)?;
            self.routers
                .get_mut(&target)
                .ok_or(NetworkError::DeviceNotFound(source))?
                .establish_bgp_session(source, BgpSessionType::IBgpPeer)?;
        }
        if update {
            self.schedule_update_router(source)?;
            self.schedule_update_router(target)?;
            self.do_queue()
        } else {
            Ok(true)
        }
    }

    /// Remove an iBGP session
    pub fn remove_ibgp_session(
        &mut self,
        source: RouterId,
        target: RouterId,
        update: bool,
    ) -> Result<bool, NetworkError> {
        self.routers
            .get_mut(&source)
            .ok_or(NetworkError::DeviceNotFound(source))?
            .close_bgp_session(target)?;
        self.routers
            .get_mut(&target)
            .ok_or(NetworkError::DeviceNotFound(target))?
            .close_bgp_session(source)?;
        if update {
            self.schedule_update_router(source)?;
            self.schedule_update_router(target)?;
            self.do_queue()
        } else {
            Ok(true)
        }
    }

    /// Write the igp forwarding tables for all internal routers. As soon as this is done, recompute
    /// the BGP table. and run the algorithm. This will happen all at once, in a very unpredictable
    /// manner. If you want to do this more predictable, use `write_ibgp_fw_table`.
    ///
    /// The function returns Ok(true) if all events caused by the igp fw table write are handled
    /// correctly. Returns Ok(false) if the max number of iterations is exceeded, and returns an
    /// error if an event was not handled correctly.
    pub fn write_igp_fw_tables(&mut self, update: bool) -> Result<bool, NetworkError> {
        // update igp table
        for r in self.routers.values_mut() {
            r.write_igp_forwarding_table(&self.net)?;
        }
        if update {
            // update bgp
            for r in self.routers.keys().cloned().collect::<Vec<RouterId>>() {
                self.schedule_update_router(r)?;
            }
            self.do_queue()
        } else {
            Ok(true)
        }
    }

    /// Write forwarding tables for the selected router
    ///
    /// The function returns Ok(true) if all events caused by the igp fw table write are handled
    /// correctly. Returns Ok(false) if the max number of iterations is exceeded, and returns an
    /// error if an event was not handled correctly.
    pub fn write_igp_fw_tables_order(
        &mut self,
        order: Vec<RouterId>,
        update: bool,
    ) -> Result<bool, NetworkError> {
        for router in order.iter() {
            self.routers
                .get_mut(&router)
                .ok_or(NetworkError::DeviceNotFound(*router))?
                .write_igp_forwarding_table(&self.net)?;
        }
        if update {
            for router in order.iter() {
                self.schedule_update_router(*router)?;
            }
            self.do_queue()
        } else {
            Ok(true)
        }
    }

    /// Advertise an external route and let the network converge
    /// The source must be a RouterId of an ExternalRouter
    pub fn advertise_external_route(
        &mut self,
        source: RouterId,
        prefix: Prefix,
        as_path: Vec<AsId>,
        med: Option<u32>,
        update: bool,
    ) -> Result<bool, NetworkError> {
        // initiate the advertisement
        println!(
            "\n*** Advertise prefix {} on {} ***\n",
            prefix.0,
            self.get_router_name(source)?
        );
        self.external_routers
            .get(&source)
            .ok_or(NetworkError::DeviceNotFound(source))?
            .advertise_prefix(prefix, as_path, med, &mut self.queue);
        if update {
            // run the queue
            self.do_queue()
        } else {
            Ok(true)
        }
    }

    /// Retract an external route and let the network converge
    /// The source must be a RouterId of an ExternalRouter
    pub fn retract_external_route(
        &mut self,
        source: RouterId,
        prefix: Prefix,
        update: bool,
    ) -> Result<bool, NetworkError> {
        println!(
            "\n*** Retract prefix {} on {} ***\n",
            prefix.0,
            self.get_router_name(source)?
        );
        // initiate the advertisement
        self.external_routers
            .get(&source)
            .ok_or(NetworkError::DeviceNotFound(source))?
            .widthdraw_prefix(prefix, &mut self.queue);
        if update {
            // run the queue
            self.do_queue()
        } else {
            Ok(true)
        }
    }

    /// Update a router and schedule the events, but dont' execute them yet
    /// Call `do_queue` to execute all the requests.
    pub fn schedule_update_router(&mut self, router: RouterId) -> Result<(), NetworkError> {
        let r = self
            .routers
            .get_mut(&router)
            .ok_or(NetworkError::DeviceNotFound(router))?;
        r.bgp_decision_process()?;
        r.bgp_route_dissemination(&mut self.queue)?;
        Ok(())
    }

    /// Execute the queue
    /// Returns Ok(false) if max iterations is exceeded
    /// Returns Ok(true) if everything was fine.
    pub fn do_queue(&mut self) -> Result<bool, NetworkError> {
        let mut remaining_iter = self.stop_after;
        while let Some(event) = self.queue.pop_front() {
            if let Some(rem) = remaining_iter {
                if rem == 0 {
                    return Ok(false);
                }
                remaining_iter = Some(rem - 1);
            }
            // print the job
            self.print_event(&event)?;
            // execute the event
            let (working_router_id, event_result) = match event {
                Event::Bgp(from, to, bgp_event) => (
                    to,
                    if let Some(r) = self.routers.get_mut(&to) {
                        r.handle_event(Event::Bgp(from, to, bgp_event), &mut self.queue)
                            .map_err(|e| NetworkError::DeviceError(e))
                    } else if let Some(r) = self.external_routers.get_mut(&to) {
                        r.handle_event(Event::Bgp(from, to, bgp_event), &mut self.queue)
                            .map_err(|e| NetworkError::DeviceError(e))
                    } else {
                        Err(NetworkError::DeviceNotFound(to))
                    },
                ),
            };

            match event_result {
                Ok(()) => {}
                Err(NetworkError::DeviceError(DeviceError::NoBgpSession(target))) => eprintln!(
                    "No BGP session active between {} and  {}!",
                    self.get_router_name(working_router_id)?,
                    self.get_router_name(target)?
                ),
                Err(e) => return Err(e),
            }
        }
        Ok(true)
    }

    /// Get an immutable reference to a router
    pub fn get_router(&mut self, router: RouterId) -> Result<&Router, NetworkError> {
        self.routers
            .get(&router)
            .ok_or(NetworkError::DeviceNotFound(router))
    }

    /// Get a mutable reference to a router
    pub fn get_router_mut(&mut self, router: RouterId) -> Result<&mut Router, NetworkError> {
        self.routers
            .get_mut(&router)
            .ok_or(NetworkError::DeviceNotFound(router))
    }

    /// return the route for the given prefix, starting at the source router.
    pub fn get_route(
        &self,
        source: RouterId,
        prefix: Prefix,
    ) -> Result<Vec<RouterId>, NetworkError> {
        // check if we are already at an external router
        if let Some(_) = self.external_routers.get(&source) {
            return Err(NetworkError::DeviceIsExternalRouter(source));
        }
        let mut visited_routers: HashSet<RouterId> = HashSet::new();
        let mut result: Vec<RouterId> = Vec::new();
        let mut current_node = source;
        loop {
            if !(self.routers.contains_key(&current_node)
                || self.external_routers.contains_key(&current_node))
            {
                return Err(NetworkError::DeviceNotFound(current_node));
            }
            result.push(current_node);
            // insert the current node into the visited routes
            if let Some(r) = self.routers.get(&current_node) {
                // we are still inside our network
                if !visited_routers.insert(current_node) {
                    return Err(NetworkError::ForwardingLoop(
                        result
                            .iter()
                            .map(|r| self.routers.get(r).unwrap().name())
                            .collect(),
                    ));
                }
                current_node = match r.get_next_hop(prefix) {
                    Some(router_id) => router_id,
                    None => {
                        return Err(NetworkError::ForwardingBlackHole(
                            result
                                .iter()
                                .map(|r| self.routers.get(r).unwrap().name())
                                .collect(),
                        ))
                    }
                };
            } else {
                break;
            }
        }
        Ok(result)
    }

    /// Print the route of a routerID to the destination
    pub fn print_route(&self, source: RouterId, prefix: Prefix) -> Result<(), NetworkError> {
        match self.get_route(source, prefix) {
            Ok(path) => println!(
                "{}",
                path.iter()
                    .map(|r| self.get_router_name(*r))
                    .collect::<Result<Vec<&'static str>, NetworkError>>()?
                    .join(" => ")
            ),
            Err(NetworkError::ForwardingLoop(path)) => {
                print!("{}", path.join(" => "));
                println!(" FORWARDING LOOP!");
            }
            Err(NetworkError::ForwardingBlackHole(path)) => {
                print!("{}", path.join(" => "));
                println!(" BLACK HOLE!");
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// print the selected egress hop for a BGP origin at a router
    pub fn print_egress_hop(&self, source: RouterId, prefix: Prefix) -> Result<(), NetworkError> {
        let r = self
            .routers
            .get(&source)
            .ok_or(NetworkError::DeviceNotFound(source))?;
        println!(
            "{} has chosen {} for {:?}",
            r.name(),
            r.get_selected_bgp_route(prefix)
                .map(|e| self.get_router_name(e.route.next_hop))
                .unwrap_or(Ok("None"))?,
            prefix
        );
        Ok(())
    }

    /// print the bgp table (known and chosen routes)
    pub fn print_bgp_table(&self, source: RouterId, prefix: Prefix) -> Result<(), NetworkError> {
        let r = self
            .routers
            .get(&source)
            .ok_or(NetworkError::DeviceNotFound(source))?;
        println!("BGP table of {} for {:?}", r.name(), prefix);
        let selected_entry = r.get_selected_bgp_route(prefix);
        let mut found = false;
        for entry in r.get_known_bgp_routes(prefix)? {
            if selected_entry.as_ref() == Some(&entry) {
                print!("* ");
                found = true;
            } else {
                print!("  ");
            }
            self.print_bgp_entry(&entry)?;
        }
        if selected_entry.is_some() && !found {
            println!("E Invalid table!");
            print!("* ");
            self.print_bgp_entry(&selected_entry.unwrap())?;
        }
        println!("");
        Ok(())
    }

    /// print a bgp route
    fn print_bgp_entry(&self, entry: &RIBEntry) -> Result<(), NetworkError> {
        print!("prefix: {}", entry.route.prefix.0);
        print!(", as_path: {:?}", entry.route.as_path);
        print!(", local_pref: {}", entry.route.local_pref.unwrap_or(100));
        print!(", MED: {}", entry.route.med.unwrap_or(0));
        print!(
            ", next_hop: {}",
            self.get_router_name(entry.route.next_hop)?
        );
        println!(", from: {}", self.get_router_name(entry.from_id)?);
        Ok(())
    }

    /// print the igp forwarding table for a specific router.
    pub fn print_igp_fw_table(&self, router_id: RouterId) -> Result<(), NetworkError> {
        let r = self
            .routers
            .get(&router_id)
            .ok_or(NetworkError::DeviceNotFound(router_id))?;
        println!("Forwarding table for {}", r.name());
        let routers_set = self
            .routers
            .keys()
            .cloned()
            .collect::<HashSet<RouterId>>()
            .union(
                &self
                    .external_routers
                    .keys()
                    .cloned()
                    .collect::<HashSet<RouterId>>(),
            )
            .cloned()
            .collect::<HashSet<RouterId>>();
        for target in routers_set {
            if let Some(Some((next_hop, cost))) = r.igp_forwarding_table.get(&target) {
                println!(
                    "  {} via {} (IGP cost: {})",
                    self.get_router_name(target)?,
                    self.get_router_name(*next_hop)?,
                    cost
                );
            } else {
                println!("  {} unreachable!", self.get_router_name(target)?);
            }
        }
        println!("");
        Ok(())
    }

    /// return the name of the router
    pub fn get_router_name(&self, router_id: RouterId) -> Result<&'static str, NetworkError> {
        if let Some(r) = self.routers.get(&router_id) {
            Ok(r.name())
        } else if let Some(r) = self.external_routers.get(&router_id) {
            Ok(r.name())
        } else {
            Err(NetworkError::DeviceNotFound(router_id))
        }
    }

    fn print_event(&self, event: &Event) -> Result<(), NetworkError> {
        match event {
            Event::Bgp(from, to, BgpEvent::Update(route)) => {
                println!(
                    "BGP Update: {} => {} {{",
                    self.get_router_name(*from)?,
                    self.get_router_name(*to)?
                );
                println!("    prefix: {}", route.prefix.0);
                println!("    as_path: {:?}", route.as_path);
                println!("    next_hop: {}", self.get_router_name(route.next_hop)?);
                println!("    local_pref: {:?}", route.local_pref);
                println!("    MED: {:?}", route.med);
                println!("}}\n");
            }
            Event::Bgp(from, to, BgpEvent::Withdraw(prefix)) => {
                println!(
                    "BGP Widthdraw: {} => {} {{",
                    self.get_router_name(*from)?,
                    self.get_router_name(*to)?
                );
                println!("    prefix: {}", prefix.0);
                println!("}}\n");
            }
        }
        Ok(())
    }
}
