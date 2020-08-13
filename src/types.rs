//! Module containing all type definitions

use crate::{Event, EventQueue};
use petgraph::prelude::*;
use petgraph::stable_graph::StableGraph;
use thiserror::Error;

type IndexType = u32;
/// Router Identification (and index into the graph)
pub type RouterId = NodeIndex<IndexType>;
/// IP Prefix (simple representation)
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct Prefix(pub u32);
/// AS Number
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub struct AsId(pub u32);
/// Link Weight for the IGP graph
pub type LinkWeight = f32;
/// IGP Network graph
pub type IgpNetwork = StableGraph<(), LinkWeight, Directed, IndexType>;

/// Trait for a network device
pub trait NetworkDevice {
    /// Create a new NetworkDevice instance
    fn new(name: &'static str, router_id: RouterId, as_id: AsId) -> Self;
    /// Handle an `Event` and produce the necessary result
    fn handle_event(&mut self, event: Event, queue: &mut EventQueue) -> Result<(), DeviceError>;
    /// Return the ID of the network device
    fn router_id(&self) -> RouterId;
    /// Return the as of the network device
    fn as_id(&self) -> AsId;
    /// Return the name of the network devcie
    fn name(&self) -> &'static str;
}

/// Router Errors
#[derive(Error, Debug, PartialEq)]
pub enum DeviceError {
    /// BGP session is already established
    #[error("BGP Session with {0:?} is already created!")]
    SessionAlreadyExists(RouterId),
    /// No BGP session is established
    #[error("BGP Session with {0:?} is not yet created!")]
    NoBgpSession(RouterId),
    /// Router was not found in the IGP forwarding table
    #[error("Router {0:?} is not known in the IGP forwarding table")]
    RouterNotFound(RouterId),
    /// Router is marked as not reachable in the IGP forwarding table.
    #[error("Router {0:?} is not reachable in IGP topology")]
    RouterNotReachable(RouterId),
}

/// Network Errors
#[derive(Error, Debug, PartialEq)]
pub enum NetworkError {
    /// Device Error which cannot be handled
    #[error("Device Error: {0}")]
    DeviceError(#[from] DeviceError),
    /// Device is not present in the topology
    #[error("Network device was not found in topology: {0:?}")]
    DeviceNotFound(RouterId),
    /// Device must be an internal router, but an external router was passed
    #[error("Netowrk device cannot be an external router: {0:?}")]
    DeviceIsExternalRouter(RouterId),
    /// Forwarding loop detected
    #[error("Forwarding Loop occurred! path: {0:?}")]
    ForwardingLoop(Vec<&'static str>),
    /// Black hole detected
    #[error("Black hole occurred! path: {0:?}")]
    ForwardingBlackHole(Vec<&'static str>),
}
