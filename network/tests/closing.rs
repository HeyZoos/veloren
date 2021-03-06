//! How to read those tests:
//!  - in the first line we call the helper, this is only debug code. in case
//!    you want to have tracing for a special test you set set the bool = true
//!    and the sleep to 10000 and your test will start 10 sec delayed with
//!    tracing. You need a delay as otherwise the other tests pollute your trace
//!  - the second line is to simulate a client and a server
//!    `network_participant_stream` will return
//!      - 2 networks
//!      - 2 participants
//!      - 2 streams
//!    each one `linked` to their counterpart.
//!    You see a cryptic use of rust `_` this is because we are testing the
//! `drop` behavior here.
//!      - A `_` means this is directly dropped after the line executes, thus
//!        immediately executing its `Drop` impl.
//!      - A `_p1_a` e.g. means we don't use that Participant yet, but we must
//!        not `drop` it yet as we might want to use the Streams.
//!  - You sometimes see sleep(1000ms) this is used when we rely on the
//!    underlying TCP functionality, as this simulates client and server

use async_std::task;
use task::block_on;
use veloren_network::{Network, ParticipantError, Pid, Promises, StreamError};
mod helper;
use helper::{network_participant_stream, tcp};

#[test]
fn close_network() {
    let (_, _) = helper::setup(false, 0);
    let (_, _p1_a, mut s1_a, _, _p1_b, mut s1_b) = block_on(network_participant_stream(tcp()));

    std::thread::sleep(std::time::Duration::from_millis(1000));

    assert_eq!(s1_a.send("Hello World"), Err(StreamError::StreamClosed));
    let msg1: Result<String, _> = block_on(s1_b.recv());
    assert_eq!(msg1, Err(StreamError::StreamClosed));
}

#[test]
fn close_participant() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, p1_a, mut s1_a, _n_b, p1_b, mut s1_b) = block_on(network_participant_stream(tcp()));

    block_on(p1_a.disconnect()).unwrap();
    block_on(p1_b.disconnect()).unwrap();

    assert_eq!(s1_a.send("Hello World"), Err(StreamError::StreamClosed));
    assert_eq!(
        block_on(s1_b.recv::<String>()),
        Err(StreamError::StreamClosed)
    );
}

#[test]
fn close_stream() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _, mut s1_a, _n_b, _, _) = block_on(network_participant_stream(tcp()));

    // s1_b is dropped directly while s1_a isn't
    std::thread::sleep(std::time::Duration::from_millis(1000));

    assert_eq!(s1_a.send("Hello World"), Err(StreamError::StreamClosed));
    assert_eq!(
        block_on(s1_a.recv::<String>()),
        Err(StreamError::StreamClosed)
    );
}

///THIS is actually a bug which currently luckily doesn't trigger, but with new
/// async-std WE must make sure, if a stream is `drop`ed inside a `block_on`,
/// that no panic is thrown.
#[test]
fn close_streams_in_block_on() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, s1_a, _n_b, _p_b, s1_b) = block_on(network_participant_stream(tcp()));
    block_on(async {
        //make it locally so that they are dropped later
        let mut s1_a = s1_a;
        let mut s1_b = s1_b;
        s1_a.send("ping").unwrap();
        assert_eq!(s1_b.recv().await, Ok("ping".to_string()));
        drop(s1_a);
    });
}

#[test]
fn stream_simple_3msg_then_close() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, mut s1_b) = block_on(network_participant_stream(tcp()));

    s1_a.send(1u8).unwrap();
    s1_a.send(42).unwrap();
    s1_a.send("3rdMessage").unwrap();
    assert_eq!(block_on(s1_b.recv()), Ok(1u8));
    assert_eq!(block_on(s1_b.recv()), Ok(42));
    assert_eq!(block_on(s1_b.recv()), Ok("3rdMessage".to_string()));
    drop(s1_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(s1_b.send("Hello World"), Err(StreamError::StreamClosed));
}

#[test]
fn stream_send_first_then_receive() {
    // recv should still be possible even if stream got closed if they are in queue
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, mut s1_b) = block_on(network_participant_stream(tcp()));

    s1_a.send(1u8).unwrap();
    s1_a.send(42).unwrap();
    s1_a.send("3rdMessage").unwrap();
    drop(s1_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(block_on(s1_b.recv()), Ok(1u8));
    assert_eq!(block_on(s1_b.recv()), Ok(42));
    assert_eq!(block_on(s1_b.recv()), Ok("3rdMessage".to_string()));
    assert_eq!(s1_b.send("Hello World"), Err(StreamError::StreamClosed));
}

#[test]
fn stream_send_1_then_close_stream() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, mut s1_b) = block_on(network_participant_stream(tcp()));
    s1_a.send("this message must be received, even if stream is closed already!")
        .unwrap();
    drop(s1_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let exp = Ok("this message must be received, even if stream is closed already!".to_string());
    assert_eq!(block_on(s1_b.recv()), exp);
    println!("all received and done");
}

#[test]
fn stream_send_100000_then_close_stream() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, mut s1_b) = block_on(network_participant_stream(tcp()));
    for _ in 0..100000 {
        s1_a.send("woop_PARTY_HARD_woop").unwrap();
    }
    drop(s1_a);
    let exp = Ok("woop_PARTY_HARD_woop".to_string());
    println!("start receiving");
    block_on(async {
        for _ in 0..100000 {
            assert_eq!(s1_b.recv().await, exp);
        }
    });
    println!("all received and done");
}

#[test]
fn stream_send_100000_then_close_stream_remote() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, _s1_b) = block_on(network_participant_stream(tcp()));
    for _ in 0..100000 {
        s1_a.send("woop_PARTY_HARD_woop").unwrap();
    }
    drop(s1_a);
    drop(_s1_b);
    //no receiving
}

#[test]
fn stream_send_100000_then_close_stream_remote2() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, _s1_b) = block_on(network_participant_stream(tcp()));
    for _ in 0..100000 {
        s1_a.send("woop_PARTY_HARD_woop").unwrap();
    }
    drop(_s1_b);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    drop(s1_a);
    //no receiving
}

#[test]
fn stream_send_100000_then_close_stream_remote3() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, _s1_b) = block_on(network_participant_stream(tcp()));
    for _ in 0..100000 {
        s1_a.send("woop_PARTY_HARD_woop").unwrap();
    }
    drop(_s1_b);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    drop(s1_a);
    //no receiving
}

#[test]
fn close_part_then_network() {
    let (_, _) = helper::setup(false, 0);
    let (n_a, p_a, mut s1_a, _n_b, _p_b, _s1_b) = block_on(network_participant_stream(tcp()));
    for _ in 0..1000 {
        s1_a.send("woop_PARTY_HARD_woop").unwrap();
    }
    drop(p_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    drop(n_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
}

#[test]
fn close_network_then_part() {
    let (_, _) = helper::setup(false, 0);
    let (n_a, p_a, mut s1_a, _n_b, _p_b, _s1_b) = block_on(network_participant_stream(tcp()));
    for _ in 0..1000 {
        s1_a.send("woop_PARTY_HARD_woop").unwrap();
    }
    drop(n_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    drop(p_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
}

#[test]
fn close_network_then_disconnect_part() {
    let (_, _) = helper::setup(false, 0);
    let (n_a, p_a, mut s1_a, _n_b, _p_b, _s1_b) = block_on(network_participant_stream(tcp()));
    for _ in 0..1000 {
        s1_a.send("woop_PARTY_HARD_woop").unwrap();
    }
    drop(n_a);
    assert!(block_on(p_a.disconnect()).is_err());
    std::thread::sleep(std::time::Duration::from_millis(1000));
}

#[test]
fn opened_stream_before_remote_part_is_closed() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, p_a, _, _n_b, p_b, _) = block_on(network_participant_stream(tcp()));
    let mut s2_a = block_on(p_a.open(10, Promises::empty())).unwrap();
    s2_a.send("HelloWorld").unwrap();
    let mut s2_b = block_on(p_b.opened()).unwrap();
    drop(p_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(block_on(s2_b.recv()), Ok("HelloWorld".to_string()));
}

#[test]
fn opened_stream_after_remote_part_is_closed() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, p_a, _, _n_b, p_b, _) = block_on(network_participant_stream(tcp()));
    let mut s2_a = block_on(p_a.open(10, Promises::empty())).unwrap();
    s2_a.send("HelloWorld").unwrap();
    drop(p_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let mut s2_b = block_on(p_b.opened()).unwrap();
    assert_eq!(block_on(s2_b.recv()), Ok("HelloWorld".to_string()));
    assert_eq!(
        block_on(p_b.opened()).unwrap_err(),
        ParticipantError::ParticipantDisconnected
    );
}

#[test]
fn open_stream_after_remote_part_is_closed() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, p_a, _, _n_b, p_b, _) = block_on(network_participant_stream(tcp()));
    let mut s2_a = block_on(p_a.open(10, Promises::empty())).unwrap();
    s2_a.send("HelloWorld").unwrap();
    drop(p_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let mut s2_b = block_on(p_b.opened()).unwrap();
    assert_eq!(block_on(s2_b.recv()), Ok("HelloWorld".to_string()));
    assert_eq!(
        block_on(p_b.open(20, Promises::empty())).unwrap_err(),
        ParticipantError::ParticipantDisconnected
    );
}

#[test]
fn failed_stream_open_after_remote_part_is_closed() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, p_a, _, _n_b, p_b, _) = block_on(network_participant_stream(tcp()));
    drop(p_a);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    assert_eq!(
        block_on(p_b.opened()).unwrap_err(),
        ParticipantError::ParticipantDisconnected
    );
}

#[test]
fn open_participant_before_remote_part_is_closed() {
    let (_, _) = helper::setup(false, 0);
    let (n_a, f) = Network::new(Pid::fake(0));
    std::thread::spawn(f);
    let (n_b, f) = Network::new(Pid::fake(1));
    std::thread::spawn(f);
    let addr = tcp();
    block_on(n_a.listen(addr.clone())).unwrap();
    let p_b = block_on(n_b.connect(addr)).unwrap();
    let mut s1_b = block_on(p_b.open(10, Promises::empty())).unwrap();
    s1_b.send("HelloWorld").unwrap();
    let p_a = block_on(n_a.connected()).unwrap();
    drop(s1_b);
    drop(p_b);
    drop(n_b);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let mut s1_a = block_on(p_a.opened()).unwrap();
    assert_eq!(block_on(s1_a.recv()), Ok("HelloWorld".to_string()));
}

#[test]
fn open_participant_after_remote_part_is_closed() {
    let (_, _) = helper::setup(false, 0);
    let (n_a, f) = Network::new(Pid::fake(0));
    std::thread::spawn(f);
    let (n_b, f) = Network::new(Pid::fake(1));
    std::thread::spawn(f);
    let addr = tcp();
    block_on(n_a.listen(addr.clone())).unwrap();
    let p_b = block_on(n_b.connect(addr)).unwrap();
    let mut s1_b = block_on(p_b.open(10, Promises::empty())).unwrap();
    s1_b.send("HelloWorld").unwrap();
    drop(s1_b);
    drop(p_b);
    drop(n_b);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let p_a = block_on(n_a.connected()).unwrap();
    let mut s1_a = block_on(p_a.opened()).unwrap();
    assert_eq!(block_on(s1_a.recv()), Ok("HelloWorld".to_string()));
}

#[test]
fn close_network_scheduler_completely() {
    let (_, _) = helper::setup(false, 0);
    let (n_a, f) = Network::new(Pid::fake(0));
    let ha = std::thread::spawn(f);
    let (n_b, f) = Network::new(Pid::fake(1));
    let hb = std::thread::spawn(f);
    let addr = tcp();
    block_on(n_a.listen(addr.clone())).unwrap();
    let p_b = block_on(n_b.connect(addr)).unwrap();
    let mut s1_b = block_on(p_b.open(10, Promises::empty())).unwrap();
    s1_b.send("HelloWorld").unwrap();

    let p_a = block_on(n_a.connected()).unwrap();
    let mut s1_a = block_on(p_a.opened()).unwrap();
    assert_eq!(block_on(s1_a.recv()), Ok("HelloWorld".to_string()));
    drop(n_a);
    drop(n_b);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    ha.join().unwrap();
    hb.join().unwrap();
}

#[test]
fn dont_panic_on_multiply_recv_after_close() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, mut s1_b) = block_on(network_participant_stream(tcp()));

    s1_a.send(11u32).unwrap();
    drop(s1_a);
    std::thread::sleep(std::time::Duration::from_secs(1));
    assert_eq!(s1_b.try_recv::<u32>(), Ok(Some(11u32)));
    assert_eq!(s1_b.try_recv::<String>(), Err(StreamError::StreamClosed));
    // There was a "Feature" in futures::channels that they panic when you call recv
    // a second time after it showed end of stream
    assert_eq!(s1_b.try_recv::<String>(), Err(StreamError::StreamClosed));
}

#[test]
fn dont_panic_on_recv_send_after_close() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, mut s1_b) = block_on(network_participant_stream(tcp()));

    s1_a.send(11u32).unwrap();
    drop(s1_a);
    std::thread::sleep(std::time::Duration::from_secs(1));
    assert_eq!(s1_b.try_recv::<u32>(), Ok(Some(11u32)));
    assert_eq!(s1_b.try_recv::<String>(), Err(StreamError::StreamClosed));
    assert_eq!(s1_b.send("foobar"), Err(StreamError::StreamClosed));
}

#[test]
fn dont_panic_on_multiple_send_after_close() {
    let (_, _) = helper::setup(false, 0);
    let (_n_a, _p_a, mut s1_a, _n_b, _p_b, mut s1_b) = block_on(network_participant_stream(tcp()));

    s1_a.send(11u32).unwrap();
    drop(s1_a);
    drop(_p_a);
    std::thread::sleep(std::time::Duration::from_secs(1));
    assert_eq!(s1_b.try_recv::<u32>(), Ok(Some(11u32)));
    assert_eq!(s1_b.try_recv::<String>(), Err(StreamError::StreamClosed));
    assert_eq!(s1_b.send("foobar"), Err(StreamError::StreamClosed));
    assert_eq!(s1_b.send("foobar"), Err(StreamError::StreamClosed));
}
