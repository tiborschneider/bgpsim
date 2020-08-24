//! Simple BGP Simulation

#![deny(missing_docs)]
#![allow(dead_code)]

mod bgp;
mod event;
mod external_router;
mod network;
mod router;
mod types;

pub use event::{EventQueue, Event};
pub use types::*;

#[cfg(test)]
mod test;

/// main function
fn main() {
    evil_twin_gadget();
}

fn evil_twin_gadget() {
    // Evil twin gadget from L. Vanbever: Improving Network Agility with Seamless BGP
    // reconfigurations
    let mut n = network::Network::new();

    // router declaration
    let r1 = n.add_router("R1");
    let r2 = n.add_router("R2");
    let r3 = n.add_router("R3");
    let r4 = n.add_router("R4");
    let ra = n.add_router("RA");
    let rb = n.add_router("RB");
    let e1 = n.add_router("E1");
    let ex = n.add_router("EX");
    let e2 = n.add_router("E2");
    let e3 = n.add_router("E3");
    let e4 = n.add_router("E4");
    let x1 = n.add_external_router("X1", AsId(65101));
    let x2 = n.add_external_router("X2", AsId(65102));
    let x3 = n.add_external_router("X3", AsId(65103));
    let x4 = n.add_external_router("X4", AsId(65104));
    let x5 = n.add_external_router("X5", AsId(65105));
    let x6 = n.add_external_router("X6", AsId(65106));

    // IGP topology
    n.add_edge(r1, e1, 2.0, None).unwrap();
    n.add_edge(r1, e2, 1.0, None).unwrap();
    n.add_edge(ra, e1, 4.0, None).unwrap();
    n.add_edge(ra, ex, 2.0, None).unwrap();
    n.add_edge(ra, e2, 3.0, None).unwrap();
    n.add_edge(r2, ex, 4.0, None).unwrap();
    n.add_edge(r2, e2, 6.0, None).unwrap();
    n.add_edge(r2, e3, 5.0, None).unwrap();
    n.add_edge(r2, e4, 3.0, None).unwrap();
    n.add_edge(rb, e1, 3.0, None).unwrap();
    n.add_edge(rb, e3, 1.0, None).unwrap();
    n.add_edge(rb, e4, 2.0, None).unwrap();
    n.add_edge(r3, e1, 8.0, None).unwrap();
    n.add_edge(r3, ex, 7.0, None).unwrap();
    n.add_edge(r3, e3, 9.0, None).unwrap();
    n.add_edge(r4, e1, 8.0, None).unwrap();
    n.add_edge(r4, e4, 9.0, None).unwrap();
    n.add_edge(r1, x1, 0.0, None).unwrap();
    n.add_edge(e1, x2, 0.0, None).unwrap();
    n.add_edge(ex, x3, 0.0, None).unwrap();
    n.add_edge(e2, x4, 0.0, None).unwrap();
    n.add_edge(e3, x5, 0.0, None).unwrap();
    n.add_edge(e4, x6, 0.0, None).unwrap();

    n.write_igp_fw_tables(true).unwrap();

    // iBGP topology
    n.add_ibgp_session(r1, e1, true, true).unwrap();
    n.add_ibgp_session(r1, ex, true, true).unwrap();
    n.add_ibgp_session(ra, e1, true, true).unwrap();
    n.add_ibgp_session(ra, ex, true, true).unwrap();
    n.add_ibgp_session(ra, e2, true, true).unwrap();
    n.add_ibgp_session(r2, ra, true, true).unwrap();
    n.add_ibgp_session(r2, e2, true, true).unwrap();
    n.add_ibgp_session(rb, e1, true, true).unwrap();
    n.add_ibgp_session(rb, e3, true, true).unwrap();
    n.add_ibgp_session(rb, e4, true, true).unwrap();
    n.add_ibgp_session(r3, rb, true, true).unwrap();
    n.add_ibgp_session(r3, e3, true, true).unwrap();
    n.add_ibgp_session(r4, e4, true, true).unwrap();
    n.add_ibgp_session(r1, r2, false, true).unwrap();
    n.add_ibgp_session(r1, r3, false, true).unwrap();
    n.add_ibgp_session(r1, r4, false, true).unwrap();
    n.add_ibgp_session(r2, r3, false, true).unwrap();
    n.add_ibgp_session(r2, r4, false, true).unwrap();
    n.add_ibgp_session(r3, r4, false, true).unwrap();

    // advertise all external sources
    n.advertise_external_route(x1, Prefix(2), vec![AsId(65101), AsId(65202)], None, true)
        .unwrap();
    n.advertise_external_route(x2, Prefix(1), vec![AsId(65102), AsId(65201)], None, true)
        .unwrap();
    n.advertise_external_route(x2, Prefix(2), vec![AsId(65102), AsId(65202)], None, true)
        .unwrap();
    n.advertise_external_route(x3, Prefix(1), vec![AsId(65103), AsId(65201)], None, true)
        .unwrap();
    n.advertise_external_route(x3, Prefix(2), vec![AsId(65103), AsId(65202)], None, true)
        .unwrap();
    n.advertise_external_route(x4, Prefix(1), vec![AsId(65104), AsId(65201)], None, true)
        .unwrap();
    n.advertise_external_route(x5, Prefix(1), vec![AsId(65105), AsId(65201)], None, true)
        .unwrap();
    n.advertise_external_route(x6, Prefix(2), vec![AsId(65106), AsId(65202)], None, true)
        .unwrap();

    // show bgp table
    n.print_bgp_table(ra, Prefix(1)).unwrap();
    n.print_bgp_table(ra, Prefix(2)).unwrap();
    n.print_bgp_table(rb, Prefix(1)).unwrap();
    n.print_bgp_table(rb, Prefix(2)).unwrap();

    // change all weights at once and recompute final state (should be ok)
    n.update_edge_weight(ra, ex, 5.0, None);
    n.update_edge_weight(rb, e3, 4.0, None);
    n.update_edge_weight(rb, e4, 5.0, None);

    std::thread::sleep(std::time::Duration::from_secs(4));

    // write igp tables and converge
    n.write_igp_fw_tables(true).unwrap();

    // slowly apply the igp update to routers one at a time
    //n.write_ibgp_fw_tables_order(vec![r1, r2, r3, r4, e1, ex, e2, e3, e4]);
    //n.write_ibgp_fw_tables_order(vec![ra]);
    //n.write_ibgp_fw_tables_order(vec![rb]);

    // show bgp table
    n.print_bgp_table(ra, Prefix(1)).unwrap();
    n.print_bgp_table(ra, Prefix(2)).unwrap();
    n.print_bgp_table(rb, Prefix(1)).unwrap();
    n.print_bgp_table(rb, Prefix(2)).unwrap();
}
