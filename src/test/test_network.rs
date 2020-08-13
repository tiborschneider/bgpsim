use crate::{network::Network, AsId, NetworkError, Prefix, RouterId};

#[test]
fn test_simple() {
    // All weights are 1
    // r0 and b0 form a iBGP cluster, and so does r1 and b1
    //
    // r0 ----- r1
    // |        |
    // |        |
    // b0       b1   internal
    // |........|............
    // |        |    external
    // e0       e1
    let mut t = Network::new();

    let prefix = Prefix(0);

    let e0 = t.add_external_router("E0", AsId(1));
    let b0 = t.add_router("B0");
    let r0 = t.add_router("R0");
    let r1 = t.add_router("R1");
    let b1 = t.add_router("B1");
    let e1 = t.add_external_router("E1", AsId(1));

    t.add_edge(e0, b0, 1.0, None).unwrap();
    t.add_edge(b0, r0, 1.0, None).unwrap();
    t.add_edge(r0, r1, 1.0, None).unwrap();
    t.add_edge(r1, b1, 1.0, None).unwrap();
    t.add_edge(b1, e1, 1.0, None).unwrap();

    t.add_ibgp_session(r0, b0, true, true).unwrap();
    t.add_ibgp_session(r1, b1, true, true).unwrap();
    t.add_ibgp_session(r0, r1, false, true).unwrap();

    t.write_igp_fw_tables(true).unwrap();

    // advertise the same prefix on both routers
    t.advertise_external_route(e0, prefix, vec![AsId(1), AsId(2), AsId(3)], None, true)
        .unwrap();
    t.advertise_external_route(e1, prefix, vec![AsId(1), AsId(2), AsId(3)], None, true)
        .unwrap();

    // check that all routes are correct
    assert_route_equal(&t, b0, prefix, vec![b0, e0]);
    assert_route_equal(&t, r0, prefix, vec![r0, b0, e0]);
    assert_route_equal(&t, r1, prefix, vec![r1, b1, e1]);
    assert_route_equal(&t, b1, prefix, vec![b1, e1]);
}

#[test]
fn test_route_order1() {
    // All weights are 1
    // r0 and b0 form a iBGP cluster, and so does r1 and b1
    //
    // r0 ----- r1
    // |        |
    // |        |
    // b1       b0   internal
    // |........|............
    // |        |    external
    // e1       e0
    let mut t = Network::new();

    let prefix = Prefix(0);

    let e0 = t.add_external_router("E0", AsId(1));
    let b0 = t.add_router("B0");
    let r0 = t.add_router("R0");
    let r1 = t.add_router("R1");
    let b1 = t.add_router("B1");
    let e1 = t.add_external_router("E1", AsId(1));

    t.add_edge(e0, b0, 1.0, None).unwrap();
    t.add_edge(b0, r1, 1.0, None).unwrap();
    t.add_edge(r0, r1, 1.0, None).unwrap();
    t.add_edge(r0, b1, 1.0, None).unwrap();
    t.add_edge(b1, e1, 1.0, None).unwrap();

    t.add_ibgp_session(r0, b0, true, true).unwrap();
    t.add_ibgp_session(r1, b1, true, true).unwrap();
    t.add_ibgp_session(r0, r1, false, true).unwrap();

    t.write_igp_fw_tables(true).unwrap();

    // advertise the same prefix on both routers
    t.advertise_external_route(e0, prefix, vec![AsId(1), AsId(2), AsId(3)], None, true)
        .unwrap();
    t.advertise_external_route(e1, prefix, vec![AsId(1), AsId(2), AsId(3)], None, true)
        .unwrap();

    // check that all routes are correct
    assert_route_equal(&t, b0, prefix, vec![b0, e0]);
    assert_route_equal(&t, r0, prefix, vec![r0, r1, b0, e0]);
    assert_route_equal(&t, r1, prefix, vec![r1, b0, e0]);
    assert_route_equal(&t, b1, prefix, vec![b1, e1]);
}

#[test]
fn test_route_order2() {
    // All weights are 1
    // r0 and b0 form a iBGP cluster, and so does r1 and b1
    //
    // r0 ----- r1
    // |        |
    // |        |
    // b1       b0   internal
    // |........|............
    // |        |    external
    // e1       e0
    let mut t = Network::new();

    let prefix = Prefix(0);

    let e0 = t.add_external_router("E0", AsId(1));
    let b0 = t.add_router("B0");
    let r0 = t.add_router("R0");
    let r1 = t.add_router("R1");
    let b1 = t.add_router("B1");
    let e1 = t.add_external_router("E1", AsId(1));

    t.add_edge(e0, b0, 1.0, None).unwrap();
    t.add_edge(b0, r1, 1.0, None).unwrap();
    t.add_edge(r0, r1, 1.0, None).unwrap();
    t.add_edge(r0, b1, 1.0, None).unwrap();
    t.add_edge(b1, e1, 1.0, None).unwrap();

    t.add_ibgp_session(r0, b0, true, true).unwrap();
    t.add_ibgp_session(r1, b1, true, true).unwrap();
    t.add_ibgp_session(r0, r1, false, true).unwrap();

    t.write_igp_fw_tables(true).unwrap();

    // advertise the same prefix on both routers
    t.advertise_external_route(e1, prefix, vec![AsId(1), AsId(2), AsId(3)], None, true)
        .unwrap();
    t.advertise_external_route(e0, prefix, vec![AsId(1), AsId(2), AsId(3)], None, true)
        .unwrap();

    // check that all routes are correct
    assert_route_equal(&t, b0, prefix, vec![b0, e0]);
    assert_route_equal(&t, r0, prefix, vec![r0, b1, e1]);
    assert_route_equal(&t, r1, prefix, vec![r1, r0, b1, e1]);
    assert_route_equal(&t, b1, prefix, vec![b1, e1]);
}

#[test]
fn test_bad_gadget() {
    // weights between ri and bi are 5, weights between ri and bi+1 are 1
    // ri and bi form a iBGP cluster
    //
    //    _________________
    //  /                  \
    // |  r0       r1       r2
    // |  | '-.    | '-.    |
    //  \ |    '-. |    '-. |
    //    b0       b1       b2   internal
    //    |........|........|............
    //    |        |        |external
    //    e0       e1       e2
    let mut t = Network::new();

    let prefix = Prefix(0);

    let e0 = t.add_external_router("E0", AsId(65100));
    let e1 = t.add_external_router("E1", AsId(65101));
    let e2 = t.add_external_router("E2", AsId(65102));
    let b0 = t.add_router("B0");
    let b1 = t.add_router("B1");
    let b2 = t.add_router("B2");
    let r0 = t.add_router("R0");
    let r1 = t.add_router("R1");
    let r2 = t.add_router("R2");

    t.add_edge(e0, b0, 1.0, None).unwrap();
    t.add_edge(e1, b1, 1.0, None).unwrap();
    t.add_edge(e2, b2, 1.0, None).unwrap();
    t.add_edge(b0, r0, 5.0, None).unwrap();
    t.add_edge(b1, r1, 5.0, None).unwrap();
    t.add_edge(b2, r2, 5.0, None).unwrap();
    t.add_edge(r0, b1, 1.0, None).unwrap();
    t.add_edge(r1, b2, 1.0, None).unwrap();
    t.add_edge(r2, b0, 1.0, None).unwrap();

    t.add_ibgp_session(r0, b0, true, true).unwrap();
    t.add_ibgp_session(r1, b1, true, true).unwrap();
    t.add_ibgp_session(r2, b2, true, true).unwrap();
    t.add_ibgp_session(r0, r1, false, true).unwrap();
    t.add_ibgp_session(r1, r2, false, true).unwrap();
    t.add_ibgp_session(r2, r0, false, true).unwrap();

    t.write_igp_fw_tables(true).unwrap();

    t.stop_after_queue(Some(1000));

    // advertise the same prefix on both routers
    assert_eq!(
        t.advertise_external_route(e2, prefix, vec![AsId(0), AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        t.advertise_external_route(e1, prefix, vec![AsId(0), AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        t.advertise_external_route(e0, prefix, vec![AsId(0), AsId(1)], None, true),
        Ok(false)
    );
}

#[test]
fn change_ibgp_topology_1() {
    // Example from L. Vanbever bgpmig_ton, figure 1
    //
    // igp topology
    //
    // rr is connected to e1, e2, e3 with weights 1, 2, 3 respectively. Assymetric: back direction has weight 100
    // ri is connected to ei with weight 10
    // ri is connected to ei-1 with weight 1
    //
    //    _________________
    //  /                  \
    // |  r3       r2       r1
    // |  | '-.    | '-.    |
    //  \ |    '-. |    '-. |
    //    e3       e2       e1   internal
    //    |........|........|............
    //    |        |        |    external
    //    p3       p2       p1
    //
    // ibgp start topology
    // .-----------------------.
    // |   rr   r1   r2   r3   | full mesh
    // '--------^----^---/^----'
    //          |    |.-' |
    //          e1   e2   e3
    //
    // ibgp end topology
    //
    //         .-rr-.
    //        /  |   \
    //       /   |    \
    //      r1   r2   r3
    //      |    |    |
    //      e1   e2   e3

    let mut n = Network::new();

    let prefix = Prefix(0);

    let rr = n.add_router("rr");
    let r1 = n.add_router("r1");
    let r2 = n.add_router("r2");
    let r3 = n.add_router("r3");
    let e1 = n.add_router("e1");
    let e2 = n.add_router("e2");
    let e3 = n.add_router("e3");
    let p1 = n.add_external_router("p1", AsId(65101));
    let p2 = n.add_external_router("p2", AsId(65102));
    let p3 = n.add_external_router("p3", AsId(65103));

    n.add_edge(r1, e1, 10.0, None).unwrap();
    n.add_edge(r2, e2, 10.0, None).unwrap();
    n.add_edge(r3, e3, 10.0, None).unwrap();
    n.add_edge(e1, p1, 1.0, None).unwrap();
    n.add_edge(e2, p2, 1.0, None).unwrap();
    n.add_edge(e3, p3, 1.0, None).unwrap();
    n.add_edge(e1, r2, 1.0, None).unwrap();
    n.add_edge(e2, r3, 1.0, None).unwrap();
    n.add_edge(e3, r1, 1.0, None).unwrap();
    n.add_edge(rr, e1, 1.0, Some(100.0)).unwrap();
    n.add_edge(rr, e2, 2.0, Some(100.0)).unwrap();
    n.add_edge(rr, e3, 3.0, Some(100.0)).unwrap();

    // start topology
    n.add_ibgp_session(rr, r1, false, true).unwrap();
    n.add_ibgp_session(rr, r2, false, true).unwrap();
    n.add_ibgp_session(rr, r3, false, true).unwrap();
    n.add_ibgp_session(r1, r2, false, true).unwrap();
    n.add_ibgp_session(r1, r3, false, true).unwrap();
    n.add_ibgp_session(r2, r3, false, true).unwrap();
    n.add_ibgp_session(r1, e1, true, true).unwrap();
    n.add_ibgp_session(r2, e2, true, true).unwrap();
    n.add_ibgp_session(r3, e3, true, true).unwrap();
    n.add_ibgp_session(r3, e2, true, true).unwrap();

    n.write_igp_fw_tables(true).unwrap();

    assert_eq!(
        n.advertise_external_route(p1, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p2, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p3, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );

    assert_route_equal(&n, r1, prefix, vec![r1, e1, p1]);
    assert_route_equal(&n, r2, prefix, vec![r2, e1, p1]);
    assert_route_equal(&n, r3, prefix, vec![r3, e2, p2]);
    assert_route_equal(&n, rr, prefix, vec![rr, e1, p1]);

    // change from the bottom up
    // modify e2
    assert_eq!(n.remove_ibgp_session(r3, e2, true), Ok(false));
}

#[test]
fn change_ibgp_topology_2() {
    // Example from L. Vanbever bgpmig_ton, figure 1
    //
    // igp topology
    //
    // rr is connected to e1, e2, e3 with weights 1, 2, 3 respectively. Assymetric: back direction
    //                               has weight 100
    // ri is connected to ei with weight 10
    // ri is connected to ei-1 with weight 1
    //
    //    _________________
    //  /                  \
    // |  r3       r2       r1
    // |  | '-.    | '-.    |
    //  \ |    '-. |    '-. |
    //    e3       e2       e1   internal
    //    |........|........|............
    //    |        |        |    external
    //    p3       p2       p1
    //
    // ibgp start topology
    // .-----------------------.
    // |   rr   r1   r2   r3   | full mesh
    // '--------^----^---/^----'
    //          |    |.-' |
    //          e1   e2   e3
    //
    // ibgp end topology
    //
    //         .-rr-.
    //        /  |   \
    //       /   |    \
    //      r1   r2   r3
    //      |    |    |
    //      e1   e2   e3

    let mut n = Network::new();

    let prefix = Prefix(0);

    let rr = n.add_router("rr");
    let r1 = n.add_router("r1");
    let r2 = n.add_router("r2");
    let r3 = n.add_router("r3");
    let e1 = n.add_router("e1");
    let e2 = n.add_router("e2");
    let e3 = n.add_router("e3");
    let p1 = n.add_external_router("p1", AsId(65101));
    let p2 = n.add_external_router("p2", AsId(65102));
    let p3 = n.add_external_router("p3", AsId(65103));

    n.add_edge(r1, e1, 10.0, None).unwrap();
    n.add_edge(r2, e2, 10.0, None).unwrap();
    n.add_edge(r3, e3, 10.0, None).unwrap();
    n.add_edge(e1, p1, 1.0, None).unwrap();
    n.add_edge(e2, p2, 1.0, None).unwrap();
    n.add_edge(e3, p3, 1.0, None).unwrap();
    n.add_edge(e1, r2, 1.0, None).unwrap();
    n.add_edge(e2, r3, 1.0, None).unwrap();
    n.add_edge(e3, r1, 1.0, None).unwrap();
    n.add_edge(rr, e1, 1.0, Some(100.0)).unwrap();
    n.add_edge(rr, e2, 2.0, Some(100.0)).unwrap();
    n.add_edge(rr, e3, 3.0, Some(100.0)).unwrap();

    // start topology
    n.add_ibgp_session(rr, r1, false, true).unwrap();
    n.add_ibgp_session(rr, r2, false, true).unwrap();
    n.add_ibgp_session(rr, r3, false, true).unwrap();
    n.add_ibgp_session(r1, r2, false, true).unwrap();
    n.add_ibgp_session(r1, r3, false, true).unwrap();
    n.add_ibgp_session(r2, r3, false, true).unwrap();
    n.add_ibgp_session(r1, e1, true, true).unwrap();
    n.add_ibgp_session(r2, e2, true, true).unwrap();
    n.add_ibgp_session(r3, e3, true, true).unwrap();
    n.add_ibgp_session(r3, e2, true, true).unwrap();

    n.write_igp_fw_tables(true).unwrap();

    assert_eq!(
        n.advertise_external_route(p1, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p2, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p3, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );

    assert_route_equal(&n, r1, prefix, vec![r1, e1, p1]);
    assert_route_equal(&n, r2, prefix, vec![r2, e1, p1]);
    assert_route_equal(&n, r3, prefix, vec![r3, e2, p2]);
    assert_route_equal(&n, rr, prefix, vec![rr, e1, p1]);

    // change from the middle routers first
    // modify r1
    assert_eq!(n.remove_ibgp_session(r1, r2, true), Ok(true));
    assert_eq!(n.remove_ibgp_session(r1, r3, true), Ok(true));
    assert_eq!(n.remove_ibgp_session(rr, r1, false), Ok(true));
    assert_eq!(n.add_ibgp_session(rr, r1, true, true), Ok(true));
    assert_route_equal(&n, r1, prefix, vec![r1, e1, p1]);
    assert_route_equal(&n, r2, prefix, vec![r2, e1, p1]);
    assert_route_equal(&n, r3, prefix, vec![r3, e2, p2]);
    assert_route_equal(&n, rr, prefix, vec![rr, e1, p1]);

    // modify r2
    assert_eq!(n.remove_ibgp_session(r2, r3, true), Ok(true));
    assert_eq!(n.remove_ibgp_session(rr, r2, false), Ok(true));
    assert_eq!(n.add_ibgp_session(rr, r2, true, true), Ok(true));
    assert_route_equal(&n, r1, prefix, vec![r1, e1, p1]);
    assert_route_equal(&n, r2, prefix, vec![r2, e1, p1]);
    assert_route_equal(&n, r3, prefix, vec![r3, e2, p2]);
    assert_route_equal(&n, rr, prefix, vec![rr, e1, p1]);

    // modify r3
    assert_eq!(n.remove_ibgp_session(rr, r3, false), Ok(true));
    assert_eq!(n.add_ibgp_session(rr, r3, true, true), Ok(true));
    assert_route_equal(&n, r1, prefix, vec![r1, e1, p1]);
    assert_route_equal(&n, r2, prefix, vec![r2, e1, p1]);
    assert_route_equal(&n, r3, prefix, vec![r3, e2, p2]);
    assert_route_equal(&n, rr, prefix, vec![rr, e1, p1]);

    // modify e2
    assert_eq!(n.remove_ibgp_session(e2, r3, true), Ok(true));
    assert_route_equal(&n, r1, prefix, vec![r1, e1, p1]);
    assert_route_equal(&n, r2, prefix, vec![r2, e1, p1]);
    assert_route_equal(&n, r3, prefix, vec![r3, e3, p3]);
    assert_route_equal(&n, rr, prefix, vec![rr, e1, p1]);
}

#[test]
fn test_pylon_gadget() {
    // Example from L. Vanbever bgpmig_ton, figure 5
    let mut n = Network::new();
    let prefix = Prefix(0);

    let s = n.add_router("s");
    let rr1 = n.add_router("rr1");
    let rr2 = n.add_router("rr2");
    let r1 = n.add_router("r1");
    let r2 = n.add_router("r2");
    let e0 = n.add_router("e0");
    let e1 = n.add_router("e1");
    let p0 = n.add_external_router("p0", AsId(65100));
    let p1 = n.add_external_router("p1", AsId(65101));
    let ps = n.add_external_router("ps", AsId(65102));

    n.add_edge(s, r1, 100.0, None).unwrap();
    n.add_edge(s, r2, 100.0, None).unwrap();
    n.add_edge(s, rr1, 100.0, None).unwrap();
    n.add_edge(s, rr2, 100.0, None).unwrap();
    n.add_edge(rr1, rr2, 1.0, None).unwrap();
    n.add_edge(rr1, e0, 1.0, None).unwrap();
    n.add_edge(rr2, e1, 1.0, None).unwrap();
    n.add_edge(r1, r2, 1.0, None).unwrap();
    n.add_edge(r1, e1, 1.0, None).unwrap();
    n.add_edge(r2, e0, 1.0, None).unwrap();
    n.add_edge(e0, p0, 1.0, None).unwrap();
    n.add_edge(e1, p1, 1.0, None).unwrap();
    n.add_edge(s, ps, 1.0, None).unwrap();

    n.add_ibgp_session(s, rr1, true, true).unwrap();
    n.add_ibgp_session(s, rr2, true, true).unwrap();
    n.add_ibgp_session(rr1, r1, true, true).unwrap();
    n.add_ibgp_session(rr2, r2, true, true).unwrap();
    n.add_ibgp_session(r1, e0, true, true).unwrap();
    n.add_ibgp_session(r2, e0, true, true).unwrap();
    n.add_ibgp_session(r2, e1, true, true).unwrap();

    n.write_igp_fw_tables(true).unwrap();

    assert_eq!(
        n.advertise_external_route(ps, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p0, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p1, prefix, vec![AsId(1)], None, true),
        Ok(true)
    );

    assert_route_equal(&n, s, prefix, vec![s, ps]);
    assert_route_equal(&n, rr1, prefix, vec![rr1, e0, p0]);
    assert_route_equal(&n, rr2, prefix, vec![rr2, rr1, e0, p0]);
    assert_route_equal(&n, r1, prefix, vec![r1, r2, e0, p0]);
    assert_route_equal(&n, r2, prefix, vec![r2, e0, p0]);

    // remove session r2 ---> e0
    assert_eq!(n.remove_ibgp_session(r2, e0, true), Ok(true));

    assert_route_equal(&n, s, prefix, vec![s, ps]);
    assert_route_equal(&n, rr1, prefix, vec![rr1, e0, p0]);
    assert_route_equal(&n, rr2, prefix, vec![rr2, e1, p1]);
    assert_route_bad(&n, r1, prefix, vec![r1, r2, r1]);
    assert_route_bad(&n, r2, prefix, vec![r2, r1, r2]);

    // add session r1 ---> e1
    assert_eq!(n.add_ibgp_session(r1, e1, true, true), Ok(true));
    assert_route_equal(&n, s, prefix, vec![s, ps]);
    assert_route_equal(&n, rr1, prefix, vec![rr1, rr2, e1, p1]);
    assert_route_equal(&n, rr2, prefix, vec![rr2, e1, p1]);
    assert_route_equal(&n, r1, prefix, vec![r1, e1, p1]);
    assert_route_equal(&n, r2, prefix, vec![r2, r1, e1, p1]);
}

#[test]
fn carousel_gadget() {
    // Example from L. Vanbever bgpmig_ton, figure 6
    let mut n = Network::new();
    let prefix1 = Prefix(1);
    let prefix2 = Prefix(2);

    let rr = n.add_router("rr");
    let r1 = n.add_router("r1");
    let r2 = n.add_router("r2");
    let r3 = n.add_router("r3");
    let r4 = n.add_router("r4");
    let e1 = n.add_router("e1");
    let e2 = n.add_router("e2");
    let e3 = n.add_router("e3");
    let e4 = n.add_router("e4");
    let pr = n.add_external_router("pr", AsId(65100));
    let p1 = n.add_external_router("p1", AsId(65101));
    let p2 = n.add_external_router("p2", AsId(65102));
    let p3 = n.add_external_router("p3", AsId(65103));
    let p4 = n.add_external_router("p4", AsId(65104));

    // make igp topology
    n.add_edge(rr, r1, 100.0, None).unwrap();
    n.add_edge(rr, r2, 100.0, None).unwrap();
    n.add_edge(rr, r3, 100.0, None).unwrap();
    n.add_edge(rr, r4, 100.0, None).unwrap();
    n.add_edge(r1, r2, 1.0, None).unwrap();
    n.add_edge(r1, e2, 5.0, None).unwrap();
    n.add_edge(r1, e3, 1.0, None).unwrap();
    n.add_edge(r2, e1, 9.0, None).unwrap();
    n.add_edge(r3, r4, 1.0, None).unwrap();
    n.add_edge(r3, e4, 9.0, None).unwrap();
    n.add_edge(r4, e2, 1.0, None).unwrap();
    n.add_edge(r4, e3, 4.0, None).unwrap();
    n.add_edge(rr, pr, 1.0, None).unwrap();
    n.add_edge(e1, p1, 1.0, None).unwrap();
    n.add_edge(e2, p2, 1.0, None).unwrap();
    n.add_edge(e3, p3, 1.0, None).unwrap();
    n.add_edge(e4, p4, 1.0, None).unwrap();

    // write fw table
    n.write_igp_fw_tables(true).unwrap();

    // make sessions
    n.add_ibgp_session(rr, r1, true, true).unwrap();
    n.add_ibgp_session(rr, r2, true, true).unwrap();
    n.add_ibgp_session(rr, r3, true, true).unwrap();
    n.add_ibgp_session(rr, r4, true, true).unwrap();
    n.add_ibgp_session(r1, e1, true, true).unwrap();
    n.add_ibgp_session(r1, e3, true, true).unwrap();
    n.add_ibgp_session(r2, e1, true, true).unwrap();
    n.add_ibgp_session(r2, e2, true, true).unwrap();
    n.add_ibgp_session(r2, e3, true, true).unwrap();
    n.add_ibgp_session(r3, e2, true, true).unwrap();
    n.add_ibgp_session(r3, e3, true, true).unwrap();
    n.add_ibgp_session(r3, e4, true, true).unwrap();
    n.add_ibgp_session(r4, e2, true, true).unwrap();
    n.add_ibgp_session(r4, e4, true, true).unwrap();

    // change the local preference for e2 and e3
    n.get_router_mut(e2)
        .unwrap()
        .policy_bgp_local_pref
        .insert(p2, 50);
    n.get_router_mut(e3)
        .unwrap()
        .policy_bgp_local_pref
        .insert(p3, 50);

    // start advertising
    assert_eq!(
        n.advertise_external_route(pr, prefix1, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(pr, prefix2, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p1, prefix1, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p2, prefix1, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p2, prefix2, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p3, prefix1, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p3, prefix2, vec![AsId(1)], None, true),
        Ok(true)
    );
    assert_eq!(
        n.advertise_external_route(p4, prefix2, vec![AsId(1)], None, true),
        Ok(true)
    );

    assert_route_equal(&n, rr, prefix1, vec![rr, pr]);
    assert_route_equal(&n, rr, prefix2, vec![rr, pr]);
    assert_route_equal(&n, r1, prefix1, vec![r1, r2, e1, p1]);
    assert_route_equal(&n, r1, prefix2, vec![r1, rr, pr]);
    assert_route_equal(&n, r2, prefix1, vec![r2, e1, p1]);
    assert_route_equal(&n, r2, prefix2, vec![r2, rr, pr]);
    assert_route_equal(&n, r3, prefix1, vec![r3, rr, pr]);
    assert_route_equal(&n, r3, prefix2, vec![r3, e4, p4]);
    assert_route_equal(&n, r4, prefix1, vec![r4, rr, pr]);
    assert_route_equal(&n, r4, prefix2, vec![r4, r3, e4, p4]);
    assert_route_equal(&n, e1, prefix1, vec![e1, p1]);
    assert_route_equal(&n, e1, prefix2, vec![e1, r2, rr, pr]);
    assert_route_equal(&n, e2, prefix1, vec![e2, r1, r2, e1, p1]);
    assert_route_equal(&n, e2, prefix2, vec![e2, r4, r3, e4, p4]);
    assert_route_equal(&n, e3, prefix1, vec![e3, r1, r2, e1, p1]);
    assert_route_equal(&n, e3, prefix2, vec![e3, r4, r3, e4, p4]);
    assert_route_equal(&n, e4, prefix1, vec![e4, r3, rr, pr]);
    assert_route_equal(&n, e4, prefix2, vec![e4, p4]);

    // reconfigure e2
    n.get_router_mut(e2)
        .unwrap()
        .policy_bgp_local_pref
        .remove(&p2);

    // schedule updates and execute
    n.schedule_update_router(e2).unwrap();
    assert_eq!(n.do_queue(), Ok(true));

    assert_route_equal(&n, rr, prefix1, vec![rr, pr]);
    assert_route_equal(&n, rr, prefix2, vec![rr, pr]);
    assert_route_bad(&n, r1, prefix1, vec![r1, r2, r1]);
    assert_route_equal(&n, r1, prefix2, vec![r1, rr, pr]);
    assert_route_bad(&n, r2, prefix1, vec![r2, r1, r2]);
    assert_route_equal(&n, r2, prefix2, vec![r2, r1, rr, pr]);
    assert_route_equal(&n, r3, prefix1, vec![r3, r4, e2, p2]);
    assert_route_equal(&n, r3, prefix2, vec![r3, r4, e2, p2]);
    assert_route_equal(&n, r4, prefix1, vec![r4, e2, p2]);
    assert_route_equal(&n, r4, prefix2, vec![r4, e2, p2]);
    assert_route_equal(&n, e1, prefix1, vec![e1, p1]);
    assert_route_equal(&n, e1, prefix2, vec![e1, r2, r1, rr, pr]);
    assert_route_equal(&n, e2, prefix1, vec![e2, p2]);
    assert_route_equal(&n, e2, prefix2, vec![e2, p2]);
    assert_route_equal(&n, e3, prefix1, vec![e3, r4, e2, p2]);
    assert_route_equal(&n, e3, prefix2, vec![e3, r4, e2, p2]);
    assert_route_equal(&n, e4, prefix1, vec![e4, r3, r4, e2, p2]);
    assert_route_equal(&n, e4, prefix2, vec![e4, p4]);

    // reconfigure e3
    n.get_router_mut(e3)
        .unwrap()
        .policy_bgp_local_pref
        .remove(&p3);

    // schedule updates and execute
    n.schedule_update_router(e3).unwrap();
    assert_eq!(n.do_queue(), Ok(true));

    assert_route_equal(&n, rr, prefix1, vec![rr, pr]);
    assert_route_equal(&n, rr, prefix2, vec![rr, pr]);
    assert_route_equal(&n, r1, prefix1, vec![r1, e3, p3]);
    assert_route_equal(&n, r1, prefix2, vec![r1, e3, p3]);
    assert_route_equal(&n, r2, prefix1, vec![r2, r1, e3, p3]);
    assert_route_equal(&n, r2, prefix2, vec![r2, r1, e3, p3]);
    assert_route_equal(&n, r3, prefix1, vec![r3, r4, e2, p2]);
    assert_route_equal(&n, r3, prefix2, vec![r3, r4, e2, p2]);
    assert_route_equal(&n, r4, prefix1, vec![r4, e2, p2]);
    assert_route_equal(&n, r4, prefix2, vec![r4, e2, p2]);
    assert_route_equal(&n, e1, prefix1, vec![e1, p1]);
    assert_route_equal(&n, e1, prefix2, vec![e1, r2, r1, e3, p3]);
    assert_route_equal(&n, e2, prefix1, vec![e2, p2]);
    assert_route_equal(&n, e2, prefix2, vec![e2, p2]);
    assert_route_equal(&n, e3, prefix1, vec![e3, p3]);
    assert_route_equal(&n, e3, prefix2, vec![e3, p3]);
    assert_route_equal(&n, e4, prefix1, vec![e4, r3, r4, e2, p2]);
    assert_route_equal(&n, e4, prefix2, vec![e4, p4]);
}

fn assert_route_equal(n: &Network, source: RouterId, prefix: Prefix, exp: Vec<RouterId>) {
    let acq = n.get_route(source, prefix);
    let exp = exp
        .iter()
        .map(|r| n.get_router_name(*r).unwrap())
        .collect::<Vec<&'static str>>();
    if let Ok(acq) = acq {
        let acq = acq
            .iter()
            .map(|r| n.get_router_name(*r).unwrap())
            .collect::<Vec<&'static str>>();
        assert_eq!(
            acq,
            exp,
            "unexpected path on {} for prefix {}:\n        acq: {:?}, exp: {:?}\n",
            n.get_router_name(source).unwrap(),
            prefix.0,
            acq,
            exp
        );
    } else if let Err(acq) = acq {
        assert_eq!(
            Err(&acq),
            Ok(&exp),
            "unexpected path on {} for prefix {}: expected good path, but got bad path!\n        acq: {:?}, exp: {:?}\n",
            n.get_router_name(source).unwrap(),
            prefix.0,
            &acq,
            &exp
        );
    }
}

fn assert_route_bad(n: &Network, source: RouterId, prefix: Prefix, exp: Vec<RouterId>) {
    let acq = n.get_route(source, prefix);
    let exp = exp
        .iter()
        .map(|r| n.get_router_name(*r).unwrap())
        .collect::<Vec<&'static str>>();
    let acq_is_ok = acq.is_ok();
    if acq_is_ok {
        let acq = acq
            .unwrap()
            .iter()
            .map(|r| n.get_router_name(*r).unwrap())
            .collect::<Vec<&'static str>>();
        assert_eq!(
            acq, exp,
            "Bad route expected on path on {} for prefix {}, but got a correct path:\n        acq: {:?}, exp: {:?}",
            n.get_router_name(source).unwrap(),
            prefix.0,
            acq,
            exp
        );
    } else {
        let acq = match acq.unwrap_err() {
            NetworkError::ForwardingLoop(x) => x,
            NetworkError::ForwardingBlackHole(x) => x,
            e => panic!("Unexpected return type: {:#?}", e),
        };
        assert_eq!(
            &acq,
            &exp,
            "Unexpected path on {} for prefix {}:\n        acq: {:?}, exp: {:?}",
            n.get_router_name(source).unwrap(),
            prefix.0,
            &acq,
            &exp
        )
    }
}
