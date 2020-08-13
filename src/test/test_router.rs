use crate::bgp::BgpSessionType::{EBgp, IBgpClient, IBgpPeer};
use crate::bgp::{BgpEvent, BgpRoute};
use crate::event::{Event, EventQueue};
use crate::router::*;
use crate::{AsId, Prefix};
use crate::{IgpNetwork, NetworkDevice};
use maplit::{hashmap, hashset};

#[test]
fn test_bgp_single() {
    let mut r = Router::new("test", 0.into(), AsId(65001));
    r.establish_bgp_session(100.into(), EBgp).unwrap();
    r.establish_bgp_session(1.into(), IBgpPeer).unwrap();
    r.establish_bgp_session(2.into(), IBgpPeer).unwrap();
    r.establish_bgp_session(3.into(), IBgpPeer).unwrap();
    r.establish_bgp_session(4.into(), IBgpClient).unwrap();
    r.establish_bgp_session(5.into(), IBgpClient).unwrap();
    r.establish_bgp_session(6.into(), IBgpClient).unwrap();
    r.igp_forwarding_table = hashmap! {
        100.into() => Some((100.into(), 0.0)),
        1.into()   => Some((1.into(), 1.0)),
        2.into()   => Some((2.into(), 1.0)),
        3.into()   => Some((2.into(), 4.0)),
        4.into()   => Some((4.into(), 2.0)),
        5.into()   => Some((4.into(), 6.0)),
        6.into()   => Some((1.into(), 13.0)),
        10.into()  => Some((1.into(), 6.0)),
        11.into()  => Some((1.into(), 15.0)),
    };

    let mut queue: EventQueue = EventQueue::new();

    /////////////////////
    // external update //
    /////////////////////

    r.handle_event(
        Event::Bgp(
            100.into(),
            0.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(200),
                as_path: vec![AsId(1), AsId(2), AsId(3), AsId(4), AsId(5)],
                next_hop: 100.into(),
                local_pref: None,
                med: None,
            }),
        ),
        &mut queue,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let entry = r.get_selected_bgp_route(Prefix(200)).unwrap();
    assert_eq!(entry.from_type, EBgp);
    assert_eq!(entry.route.next_hop, 100.into());
    assert_eq!(entry.route.local_pref, Some(100));
    assert_eq!(queue.len(), 6);
    while let Some(job) = queue.pop_front() {
        match job {
            Event::Bgp(from, _, BgpEvent::Update(r)) => {
                assert_eq!(from, 0.into());
                assert_eq!(r.next_hop, 100.into());
            }
            _ => assert!(false),
        }
    }
    // used for later
    let original_entry = entry.clone();

    /////////////////////
    // internal update //
    /////////////////////

    // update from route reflector

    r.handle_event(
        Event::Bgp(
            1.into(),
            0.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(201),
                as_path: vec![AsId(1), AsId(2), AsId(3)],
                next_hop: 11.into(),
                local_pref: Some(50),
                med: None,
            }),
        ),
        &mut queue,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let entry = r.get_selected_bgp_route(Prefix(201)).unwrap();
    assert_eq!(entry.from_type, IBgpPeer);
    assert_eq!(entry.route.next_hop, 11.into());
    assert_eq!(entry.route.local_pref, Some(50));
    assert_eq!(queue.len(), 4);
    while let Some(job) = queue.pop_front() {
        match job {
            Event::Bgp(from, to, BgpEvent::Update(r)) => {
                assert_eq!(from, 0.into());
                assert!(hashset![4, 5, 6, 100].contains(&(to.index() as usize)));
                if to == 100.into() {
                    assert_eq!(r.next_hop, 0.into());
                } else {
                    assert_eq!(r.next_hop, 11.into());
                }
            }
            _ => assert!(false),
        }
    }

    //////////////////
    // worse update //
    //////////////////

    // update from route reflector

    r.handle_event(
        Event::Bgp(
            2.into(),
            0.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(200),
                as_path: vec![AsId(1), AsId(2), AsId(3), AsId(4), AsId(5)],
                next_hop: 10.into(),
                local_pref: None,
                med: None,
            }),
        ),
        &mut queue,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let entry = r.get_selected_bgp_route(Prefix(200)).unwrap();
    assert_eq!(entry.from_type, EBgp);
    assert_eq!(entry.route.next_hop, 100.into());
    assert_eq!(queue.len(), 0);

    ///////////////////
    // better update //
    ///////////////////

    // update from route reflector

    r.handle_event(
        Event::Bgp(
            5.into(),
            0.into(),
            BgpEvent::Update(BgpRoute {
                prefix: Prefix(200),
                as_path: vec![
                    AsId(1),
                    AsId(2),
                    AsId(3),
                    AsId(4),
                    AsId(5),
                    AsId(6),
                    AsId(7),
                    AsId(8),
                    AsId(9),
                    AsId(10),
                ],
                next_hop: 5.into(),
                local_pref: Some(150),
                med: None,
            }),
        ),
        &mut queue,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let entry = r.get_selected_bgp_route(Prefix(200)).unwrap().clone();
    assert_eq!(entry.from_type, IBgpClient);
    assert_eq!(entry.route.next_hop, 5.into());
    assert_eq!(entry.route.local_pref, Some(150));
    assert_eq!(queue.len(), 7);
    while let Some(job) = queue.pop_front() {
        match job {
            Event::Bgp(from, to, BgpEvent::Update(r)) => {
                assert_eq!(from, 0.into());
                assert!(hashset![1, 2, 3, 4, 6, 100].contains(&(to.index() as usize)));
                if to == 100.into() {
                    assert_eq!(r.next_hop, 0.into());
                    assert_eq!(r.local_pref, None);
                } else {
                    assert_eq!(r.next_hop, 5.into());
                    assert_eq!(r.local_pref, Some(150));
                }
            }
            Event::Bgp(from, to, BgpEvent::Withdraw(prefix)) => {
                assert_eq!(from, 0.into());
                assert_eq!(to, 5.into());
                assert_eq!(prefix, Prefix(200));
            }
        }
    }

    ///////////////////////
    // retract bad route //
    ///////////////////////

    r.handle_event(
        Event::Bgp(2.into(), 0.into(), BgpEvent::Withdraw(Prefix(200))),
        &mut queue,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    let new_entry = r.get_selected_bgp_route(Prefix(200)).unwrap();
    assert_eq!(new_entry, entry);
    assert_eq!(queue.len(), 0);

    ////////////////////////
    // retract good route //
    ////////////////////////

    r.handle_event(
        Event::Bgp(5.into(), 0.into(), BgpEvent::Withdraw(Prefix(200))),
        &mut queue,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    //eprintln!("{:#?}", r);
    let new_entry = r.get_selected_bgp_route(Prefix(200)).unwrap();
    assert_eq!(new_entry, original_entry);
    assert_eq!(queue.len(), 7);
    while let Some(job) = queue.pop_front() {
        match job {
            Event::Bgp(from, to, BgpEvent::Update(r)) => {
                assert_eq!(from, 0.into());
                assert!(hashset![1, 2, 3, 4, 5, 6].contains(&(to.index() as usize)));
                assert_eq!(r.next_hop, 100.into());
                assert_eq!(r.local_pref, Some(100));
            }
            Event::Bgp(from, to, BgpEvent::Withdraw(prefix)) => {
                assert_eq!(from, 0.into());
                assert_eq!(to, 100.into());
                assert_eq!(prefix, Prefix(200));
            }
        }
    }

    ////////////////////////
    // retract last route //
    ////////////////////////

    r.handle_event(
        Event::Bgp(100.into(), 0.into(), BgpEvent::Withdraw(Prefix(200))),
        &mut queue,
    )
    .unwrap();

    // check that the router now has a route selected for 100 with the correct data
    assert!(r.get_selected_bgp_route(Prefix(200)).is_none());
    assert_eq!(queue.len(), 6);
    while let Some(job) = queue.pop_front() {
        match job {
            Event::Bgp(from, to, BgpEvent::Withdraw(Prefix(200))) => {
                assert_eq!(from, 0.into());
                assert!(hashset![1, 2, 3, 4, 5, 6].contains(&(to.index() as usize)));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_fw_table_simple() {
    let mut net: IgpNetwork = IgpNetwork::new();
    let mut a = Router::new("A", net.add_node(()), AsId(65001));
    let mut b = Router::new("B", net.add_node(()), AsId(65001));
    let mut c = Router::new("C", net.add_node(()), AsId(65001));
    let d = Router::new("D", net.add_node(()), AsId(65001));
    let e = Router::new("E", net.add_node(()), AsId(65001));

    net.add_edge(a.router_id(), b.router_id(), 1.0);
    net.add_edge(b.router_id(), c.router_id(), 1.0);
    net.add_edge(c.router_id(), d.router_id(), 1.0);
    net.add_edge(d.router_id(), e.router_id(), 1.0);
    net.add_edge(e.router_id(), d.router_id(), 1.0);
    net.add_edge(d.router_id(), c.router_id(), 1.0);
    net.add_edge(c.router_id(), b.router_id(), 1.0);
    net.add_edge(b.router_id(), a.router_id(), 1.0);

    /*
     * all weights = 1
     * c ----- c
     * |       |
     * |       |
     * b       d
     * |       |
     * |       |
     * a       e
     */

    a.write_igp_forwarding_table(&net).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((a.router_id(), 0.0)),
        b.router_id() => Some((b.router_id(), 1.0)),
        c.router_id() => Some((b.router_id(), 2.0)),
        d.router_id() => Some((b.router_id(), 3.0)),
        e.router_id() => Some((b.router_id(), 4.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &a.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }

    b.write_igp_forwarding_table(&net).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((a.router_id(), 1.0)),
        b.router_id() => Some((b.router_id(), 0.0)),
        c.router_id() => Some((c.router_id(), 1.0)),
        d.router_id() => Some((c.router_id(), 2.0)),
        e.router_id() => Some((c.router_id(), 3.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &b.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }

    c.write_igp_forwarding_table(&net).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((b.router_id(), 2.0)),
        b.router_id() => Some((b.router_id(), 1.0)),
        c.router_id() => Some((c.router_id(), 0.0)),
        d.router_id() => Some((d.router_id(), 1.0)),
        e.router_id() => Some((d.router_id(), 2.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &c.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }
}

#[test]
fn test_igp_fw_table_complex() {
    let mut net: IgpNetwork = IgpNetwork::new();
    let mut a = Router::new("A", net.add_node(()), AsId(65001));
    let b = Router::new("B", net.add_node(()), AsId(65001));
    let mut c = Router::new("C", net.add_node(()), AsId(65001));
    let d = Router::new("D", net.add_node(()), AsId(65001));
    let e = Router::new("E", net.add_node(()), AsId(65001));
    let f = Router::new("F", net.add_node(()), AsId(65001));
    let g = Router::new("G", net.add_node(()), AsId(65001));
    let h = Router::new("H", net.add_node(()), AsId(65001));

    net.add_edge(a.router_id(), b.router_id(), 3.0);
    net.add_edge(b.router_id(), a.router_id(), 3.0);
    net.add_edge(a.router_id(), e.router_id(), 1.0);
    net.add_edge(e.router_id(), a.router_id(), 1.0);
    net.add_edge(b.router_id(), c.router_id(), 8.0);
    net.add_edge(c.router_id(), b.router_id(), 8.0);
    net.add_edge(b.router_id(), f.router_id(), 2.0);
    net.add_edge(f.router_id(), b.router_id(), 2.0);
    net.add_edge(c.router_id(), d.router_id(), 8.0);
    net.add_edge(d.router_id(), c.router_id(), 8.0);
    net.add_edge(c.router_id(), f.router_id(), 1.0);
    net.add_edge(f.router_id(), c.router_id(), 1.0);
    net.add_edge(c.router_id(), g.router_id(), 1.0);
    net.add_edge(g.router_id(), c.router_id(), 1.0);
    net.add_edge(d.router_id(), h.router_id(), 1.0);
    net.add_edge(h.router_id(), d.router_id(), 1.0);
    net.add_edge(e.router_id(), f.router_id(), 1.0);
    net.add_edge(f.router_id(), e.router_id(), 1.0);
    net.add_edge(f.router_id(), g.router_id(), 8.0);
    net.add_edge(g.router_id(), f.router_id(), 8.0);
    net.add_edge(g.router_id(), h.router_id(), 1.0);
    net.add_edge(h.router_id(), g.router_id(), 1.0);

    /*
     *    3      8      8
     * a ---- b ---- c ---- d
     * |      |    / |      |
     * |1    2|  --  |1     |1
     * |      | / 1  |      |
     * e ---- f ---- g ---- h
     *    1      8      1
     */

    a.write_igp_forwarding_table(&net).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((a.router_id(), 0.0)),
        b.router_id() => Some((b.router_id(), 3.0)),
        c.router_id() => Some((e.router_id(), 3.0)),
        d.router_id() => Some((e.router_id(), 6.0)),
        e.router_id() => Some((e.router_id(), 1.0)),
        f.router_id() => Some((e.router_id(), 2.0)),
        g.router_id() => Some((e.router_id(), 4.0)),
        h.router_id() => Some((e.router_id(), 5.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &a.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e, &f, &g, &h] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }

    c.write_igp_forwarding_table(&net).unwrap();

    let expected_forwarding_table = hashmap! {
        a.router_id() => Some((f.router_id(), 3.0)),
        b.router_id() => Some((f.router_id(), 3.0)),
        c.router_id() => Some((c.router_id(), 0.0)),
        d.router_id() => Some((g.router_id(), 3.0)),
        e.router_id() => Some((f.router_id(), 2.0)),
        f.router_id() => Some((f.router_id(), 1.0)),
        g.router_id() => Some((g.router_id(), 1.0)),
        h.router_id() => Some((g.router_id(), 2.0)),
    };

    let exp = &expected_forwarding_table;
    let acq = &c.igp_forwarding_table;

    for target in vec![&a, &b, &c, &d, &e, &f, &g, &h] {
        assert_eq!(exp.get(&target.router_id()), acq.get(&target.router_id()));
    }
}
