/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `sys/socket.h` (Sockets)
//!
//! We currently support blocking TCP and UDP guest sockets on IPv4 addresses.
//!
//! Because fine grain control is needed, those are implemented as
//! _non-blocking_ host sockets. Moreover, app usage of select() is
//! (optimistically) assumed to check for data readiness before calling
//! any of blocking functions.
//! (Check related functions for more details and remediation.)
//!
//! Other note: Rust std::net APIs are "too high level" sometimes,
//! thus some workarounds need to be implemented.
//! (e.g. [TcpListener] does both bind() and listen() on a call
//! to [TcpListener::bind])
//!
//! Useful resources:
//! - [Beej's Guide to Network Programming](https://beej.us/guide/bgnet/html/index-wide.html)

use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::errno::set_errno;
use crate::libc::posix_io::{find_or_create_socket, FileDescriptor};
use crate::libc::time::timeval;
use crate::mem::{guest_size_of, ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr, SafeRead};
use crate::Environment;

use crate::libc::netdb::{socklen_t, IPPROTO_TCP, IPPROTO_UDP};
use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket};

pub const AF_INET: i32 = 2;
pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;

const SOL_SOCKET: i32 = 0xffff;
const SO_REUSEADDR: i32 = 0x4;
const SO_BROADCAST: i32 = 0x20;

#[allow(non_camel_case_types)]
pub type sa_family_t = u8;

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
pub struct sockaddr {
    sa_len: u8,
    sa_family: sa_family_t,
    sa_data: [u8; 14],
}
unsafe impl SafeRead for sockaddr {}
impl sockaddr {
    /// Makes an IPv4 sockaddr from 4 bytes for ip and a port.
    ///
    /// Port is expected to be native endian and
    /// will be converted to big endian internally.
    pub fn from_ipv4_parts(octets: [u8; 4], port: u16) -> Self {
        let mut addr = sockaddr {
            sa_len: 16,
            sa_family: AF_INET as u8,
            sa_data: [0; 14],
        };
        addr.sa_data[0..2].copy_from_slice(&port.to_be_bytes());
        addr.sa_data[2..6].copy_from_slice(&octets);
        addr
    }
    /// Returns 4 bytes for ip and a port.
    ///
    /// Port is returned in the native endian format.
    fn to_ipv4_parts(self) -> ([u8; 4], u16) {
        assert!(self.sa_len == 16 || self.sa_len == 0);
        assert_eq!(self.sa_family, AF_INET as u8);
        let port = u16::from_be_bytes([self.sa_data[0], self.sa_data[1]]);
        let ip = [
            self.sa_data[2],
            self.sa_data[3],
            self.sa_data[4],
            self.sa_data[5],
        ];
        (ip, port)
    }
    fn to_sockaddr_v4(self) -> SocketAddrV4 {
        let (ip, port) = self.to_ipv4_parts();
        SocketAddrV4::new(ip.into(), port)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
#[allow(non_camel_case_types)]
pub struct fd_set {
    // 32 4-byte ints should be enough for 1024 file descriptors
    fds_bits: [i32; 32],
}
unsafe impl SafeRead for fd_set {}

struct SocketHostObject {
    /// Type of the socket, [SOCK_STREAM] for TCP or [SOCK_DGRAM] for UDP
    type_: i32,
    /// Set of options
    options: HashSet<i32>,
    /// TCP socket which is yet to be connected
    tcp_listener: Option<TcpListener>,
    /// Already connected TCP socket
    tcp_stream: Option<TcpStream>,
    /// UDP socket
    udp_socket: Option<UdpSocket>,
}

#[derive(Default)]
pub struct State {
    sockets: HashMap<i32, SocketHostObject>,
}
impl State {
    fn get(env: &mut Environment) -> &Self {
        &env.libc_state.socket
    }
    fn get_mut(env: &mut Environment) -> &mut Self {
        &mut env.libc_state.socket
    }
}

fn socket(env: &mut Environment, domain: i32, type_: i32, protocol: i32) -> FileDescriptor {
    // TODO: handle errno properly
    set_errno(env, 0);

    assert_eq!(domain, AF_INET);
    assert!(type_ == SOCK_STREAM || type_ == SOCK_DGRAM);
    assert!(protocol == IPPROTO_TCP || protocol == IPPROTO_UDP || protocol == 0);

    let fd = find_or_create_socket(env);
    assert!(!State::get(env).sockets.contains_key(&fd));
    let host_object = SocketHostObject {
        type_,
        options: Default::default(),
        tcp_listener: None,
        tcp_stream: None,
        udp_socket: None,
    };
    State::get_mut(env).sockets.insert(fd, host_object);

    log_dbg!("socket({}, {}, {}) => {}", domain, type_, protocol, fd);
    fd
}

fn setsockopt(
    env: &mut Environment,
    socket: i32,
    level: i32,
    option_name: i32,
    option_value: ConstVoidPtr,
    option_len: socklen_t,
) -> i32 {
    let type_ = State::get(env).sockets.get(&socket).unwrap().type_;
    assert!(type_ == SOCK_STREAM || type_ == SOCK_DGRAM);

    assert_eq!(level, SOL_SOCKET);
    // TODO: SO_REUSEADDR is not supported in std::net (and not so portable)
    assert!(option_name == SO_REUSEADDR || option_name == SO_BROADCAST);
    log_dbg!(
        "setsockopt({}, {}, {:?}, {:?}, {})",
        socket,
        level,
        option_name,
        option_value,
        option_len
    );
    assert_eq!(option_len, guest_size_of::<i32>());
    let tmp: ConstPtr<i32> = option_value.cast();
    assert_eq!(env.mem.read(tmp), 1);

    let options = &mut State::get_mut(env)
        .sockets
        .get_mut(&socket)
        .unwrap()
        .options;
    options.insert(option_name);

    0 // Success
}

fn bind(
    env: &mut Environment,
    socket: i32,
    address: ConstPtr<sockaddr>,
    address_len: socklen_t,
) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    let type_ = State::get(env).sockets.get(&socket).unwrap().type_;
    assert!(type_ == SOCK_STREAM || type_ == SOCK_DGRAM);

    assert_eq!(address_len, guest_size_of::<sockaddr>());
    let sockaddr_val = env.mem.read(address);
    log_dbg!("bind({:?} ({:?}), {})", address, sockaddr_val, address_len);

    let socket_address = sockaddr_val.to_sockaddr_v4();
    let type_str = match type_ {
        SOCK_STREAM => "TCP",
        SOCK_DGRAM => "UDP",
        _ => unreachable!(),
    };
    log_dbg!("bind: {} socket address {:?}", type_str, socket_address);

    match type_ {
        SOCK_STREAM => {
            assert!(State::get(env)
                .sockets
                .get(&socket)
                .unwrap()
                .tcp_listener
                .is_none());
            let host_socket = TcpListener::bind(socket_address).unwrap();
            // We set host socket as non-blocking in order to have
            // more control of how and when it's used
            host_socket.set_nonblocking(true).unwrap();
            // TODO: set options
            State::get_mut(env)
                .sockets
                .get_mut(&socket)
                .unwrap()
                .tcp_listener = Some(host_socket);
        }
        SOCK_DGRAM => {
            assert!(State::get(env)
                .sockets
                .get(&socket)
                .unwrap()
                .udp_socket
                .is_none());
            let host_socket = UdpSocket::bind(socket_address).unwrap();
            // We set host socket as non-blocking in order to have
            // more control of how and when it's used
            host_socket.set_nonblocking(true).unwrap();
            for &option in &State::get(env).sockets.get(&socket).unwrap().options {
                if option == SO_BROADCAST {
                    host_socket.set_broadcast(true).unwrap();
                }
            }
            State::get_mut(env)
                .sockets
                .get_mut(&socket)
                .unwrap()
                .udp_socket = Some(host_socket);
        }
        _ => unreachable!(),
    }

    0 // Success
}

fn listen(env: &mut Environment, socket: i32, backlog: i32) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    let type_ = State::get(env).sockets.get(&socket).unwrap().type_;
    assert!(type_ == SOCK_STREAM);

    log!(
        "Warning: listen(socket: {}, backlog: {}), ignoring",
        socket,
        backlog
    );
    0 // Success
}

fn connect(
    env: &mut Environment,
    socket: i32,
    address: ConstPtr<sockaddr>,
    address_len: socklen_t,
) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    let type_ = State::get(env).sockets.get(&socket).unwrap().type_;
    assert!(type_ == SOCK_STREAM);

    assert_eq!(address_len, guest_size_of::<sockaddr>());
    let sockaddr_val = env.mem.read(address);
    log_dbg!(
        "connect({:?} ({:?}), {})",
        address,
        sockaddr_val,
        address_len
    );

    let socket_address = sockaddr_val.to_sockaddr_v4();
    log_dbg!("connect: socket address {:?}", socket_address);

    assert!(State::get(env)
        .sockets
        .get(&socket)
        .unwrap()
        .tcp_stream
        .is_none());
    let host_stream = TcpStream::connect(socket_address).unwrap();
    // We set host socket as non-blocking in order to have
    // more control of how and when it's used
    host_stream.set_nonblocking(true).unwrap();
    State::get_mut(env)
        .sockets
        .get_mut(&socket)
        .unwrap()
        .tcp_stream = Some(host_stream);

    0 // Success
}

fn select(
    env: &mut Environment,
    n_fds: i32,
    read_fds: MutPtr<fd_set>,
    write_fds: MutPtr<fd_set>,
    error_fds: MutPtr<fd_set>,
    timeout: MutPtr<timeval>,
) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    // TODO: other type of sets
    assert!(write_fds.is_null());
    assert!(error_fds.is_null());

    let timeval = env.mem.read(timeout);
    let tv_sec = timeval.tv_sec;
    assert_eq!(tv_sec, 0); // TODO
    let tv_usec = timeval.tv_usec;
    assert_eq!(tv_usec, 0); // TODO

    let mut read_set = env.mem.read(read_fds);
    log_dbg!("select: read_set before {:?}", read_set);
    let mut fds_bits = read_set.fds_bits;
    let mut count = 0;
    'outer: for (i, bits) in fds_bits.iter_mut().enumerate() {
        for bit_index in 0..32i32 {
            let fd: FileDescriptor = (i as i32) * 32 + bit_index;
            if fd > n_fds {
                break 'outer;
            }
            if (*bits & (1 << bit_index)) != 0 {
                log_dbg!("select: bit set at fd: {}", fd);
                // Clean bit in the set for the current socket
                *bits &= !(1 << bit_index);
                let type_ = State::get(env).sockets.get(&fd).unwrap().type_;
                match type_ {
                    SOCK_DGRAM => {
                        let udp_socket = State::get(env)
                            .sockets
                            .get(&fd)
                            .unwrap()
                            .udp_socket
                            .as_ref()
                            .unwrap();
                        // TODO: how many bytes we should peek?
                        let mut buf = [0; 1];
                        match udp_socket.peek(&mut buf) {
                            Ok(received) => {
                                log_dbg!("select: Socket {} peeked {} bytes", fd, received);
                                // Set bit back
                                *bits |= 1 << bit_index;
                                count += 1;
                                continue;
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                log_dbg!("select: Socket {} would block on peeking, continue.", fd);
                                continue;
                            }
                            Err(e) => {
                                panic!("select: Peek for socket {} failed: {e:?}", fd)
                            }
                        }
                    }
                    SOCK_STREAM => {
                        if State::get(env)
                            .sockets
                            .get(&fd)
                            .unwrap()
                            .tcp_stream
                            .is_none()
                        {
                            // If we don't have a TCP stream it probably means
                            // that a listener is waiting for connection
                            let listener = State::get(env)
                                .sockets
                                .get(&fd)
                                .unwrap()
                                .tcp_listener
                                .as_ref()
                                .unwrap();
                            // The listener is non-blocking,
                            // so we can try to accept
                            match listener.accept() {
                                Ok((_, addr)) => {
                                    unimplemented!("select: New client: {}", addr)
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    // No incoming connection is ready
                                    log_dbg!("select: TCP listener for socket {} would block on accepting, continue.", fd);
                                    continue;
                                }
                                Err(e) => {
                                    panic!(
                                        "select: Socket {} has error accepting connection: {}",
                                        fd, e
                                    );
                                }
                            }
                        }
                        let stream = State::get(env)
                            .sockets
                            .get(&fd)
                            .unwrap()
                            .tcp_stream
                            .as_ref()
                            .unwrap();
                        // TODO: how many bytes we should peek?
                        let mut buf = [0; 1];
                        match stream.peek(&mut buf) {
                            Ok(received) => {
                                unimplemented!("select: received {} bytes", received)
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                log_dbg!("select: TCP stream for socket {} would block on peeking, continue.", fd);
                            }
                            Err(e) => {
                                panic!("select: Peek for socket {} failed: {}", fd, e)
                            }
                        }
                    }
                    _ => unimplemented!(),
                }
            }
        }
    }
    read_set.fds_bits = fds_bits;
    log_dbg!("select: read_set after {:?}", read_set);
    env.mem.write(read_fds, read_set);
    count
}

fn accept(
    env: &mut Environment,
    socket: i32,
    _addr: MutPtr<sockaddr>,
    _addr_len: MutPtr<socklen_t>,
) -> i32 {
    let type_ = State::get(env).sockets.get(&socket).unwrap().type_;
    assert!(type_ == SOCK_STREAM);

    let listener = State::get(env)
        .sockets
        .get(&socket)
        .unwrap()
        .tcp_listener
        .as_ref()
        .unwrap();
    match listener.accept() {
        Ok((_, addr)) => {
            log!("accept: New client: {}", addr);
            unimplemented!()
        }
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            // No incoming connection is ready
            // TODO: if this happened, take a deep breath and do:
            // - block guest thread with a new [ThreadBlock] type
            // - poll for data in thread scheduling part
            // - write/read/accept/etc data once it is ready
            // - unblock guest thread
            unimplemented!("accept: TCP listener for socket {} would block on accepting, block current guest thread {}.", socket, env.current_thread)
        }
        Err(e) => {
            panic!(
                "accept: Socket {} has error accepting connection: {}",
                socket, e
            );
        }
    }
}

fn recvfrom(
    env: &mut Environment,
    socket: i32,
    buffer: MutVoidPtr,
    length: GuestUSize,
    flags: i32,
    address: MutPtr<sockaddr>,
    address_len: MutPtr<socklen_t>,
) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    let type_ = State::get(env).sockets.get(&socket).unwrap().type_;
    assert!(type_ == SOCK_STREAM || type_ == SOCK_DGRAM);

    assert_eq!(flags, 0); // TODO

    let (num_bytes_read, addr) = match type_ {
        SOCK_DGRAM => {
            let udp_socket = env
                .libc_state
                .socket
                .sockets
                .get(&socket)
                .unwrap()
                .udp_socket
                .as_ref()
                .unwrap();
            let buf = env.mem.bytes_at_mut(buffer.cast(), length);
            let (read, addr) = match udp_socket.recv_from(buf) {
                Ok(n) => n,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No data is ready
                    // TODO: if this happened, take a deep breath and do:
                    // - block guest thread with a new [ThreadBlock] type
                    // - poll for data in thread scheduling part
                    // - write/read/accept/etc data once it is ready
                    // - unblock guest thread
                    unimplemented!("recvfrom: UDP socket {} would block on receiving, block current guest thread {}.", socket, env.current_thread)
                }
                Err(e) => panic!("recvfrom: Socket {} encountered IO error: {}", socket, e),
            };
            (read, addr)
        }
        _ => unimplemented!(),
    };
    log_dbg!(
        "recvfrom: Socket {} received {} bytes from addr {:?}",
        socket,
        num_bytes_read,
        addr
    );

    if !address.is_null() {
        // Only IPV4 for the moment
        assert!(addr.is_ipv4());
        let SocketAddr::V4(ipv4addr) = addr else {
            unreachable!()
        };
        assert_eq!(guest_size_of::<sockaddr>(), env.mem.read(address_len));
        let guest_addr = sockaddr::from_ipv4_parts(ipv4addr.ip().octets(), ipv4addr.port());
        env.mem.write(address, guest_addr);
        env.mem.write(address_len, guest_size_of::<sockaddr>());
    }
    num_bytes_read.try_into().unwrap()
}

fn sendto(
    env: &mut Environment,
    socket: i32,
    buffer: MutVoidPtr,
    length: GuestUSize,
    flags: i32,
    dest_address: MutPtr<sockaddr>,
    dest_address_len: socklen_t,
) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    let type_ = State::get(env).sockets.get(&socket).unwrap().type_;
    assert!(type_ == SOCK_STREAM || type_ == SOCK_DGRAM);

    assert_eq!(flags, 0); // TODO

    assert_eq!(dest_address_len, guest_size_of::<sockaddr>());
    let sockaddr_val = env.mem.read(dest_address);
    log_dbg!(
        "sendto({}, {:?}. {}, {}, {:?} ({:?}), {})",
        socket,
        buffer,
        length,
        flags,
        dest_address,
        sockaddr_val,
        dest_address_len
    );

    let socket_address = sockaddr_val.to_sockaddr_v4();
    let type_str = match type_ {
        SOCK_STREAM => "TCP",
        SOCK_DGRAM => "UDP",
        _ => unreachable!(),
    };
    log_dbg!("sendto: {} socket address {:?}", type_str, socket_address);

    let num_bytes_written = match type_ {
        SOCK_DGRAM => {
            let udp_socket = env
                .libc_state
                .socket
                .sockets
                .get(&socket)
                .unwrap()
                .udp_socket
                .as_ref()
                .unwrap();
            let buf = env.mem.bytes_at(buffer.cast(), length);
            match udp_socket.send_to(buf, socket_address) {
                Ok(written) => written,
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // TODO: if this happened, take a deep breath and do:
                    // - block guest thread with a new [ThreadBlock] type
                    // - poll for data in thread scheduling part
                    // - write/read/accept/etc data once it is ready
                    // - unblock guest thread
                    unimplemented!("sendto: UDP socket {} would block on sending, block current guest thread {}.", socket, env.current_thread)
                }
                Err(e) => panic!("sendto: Socket {} encountered IO error: {}", socket, e),
            }
        }
        _ => unimplemented!(),
    };
    num_bytes_written.try_into().unwrap()
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(socket(_, _, _)),
    export_c_func!(setsockopt(_, _, _, _, _)),
    export_c_func!(bind(_, _, _)),
    export_c_func!(listen(_, _)),
    export_c_func!(connect(_, _, _)),
    export_c_func!(select(_, _, _, _, _)),
    export_c_func!(accept(_, _, _)),
    export_c_func!(recvfrom(_, _, _, _, _, _)),
    export_c_func!(sendto(_, _, _, _, _, _)),
];

/// A helper to close a socket, not a part of API
pub fn close_socket(env: &mut Environment, socket: i32) -> bool {
    State::get_mut(env).sockets.remove(&socket).is_none()
}
