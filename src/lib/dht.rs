extern crate bencode;
extern crate byteorder;
extern crate rand;
extern crate sha1;

use rand::prelude::*;
use self::bencode::{Bencode, FromBencode, ToBencode};
use self::bencode::util::ByteString;
use self::byteorder::{BigEndian, ReadBytesExt};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::net;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const BOOTSTRAP_NODES: [&str; 3] = [
    "router.bittorrent.com:6881",
    "dht.transmissionbt.com:6881",
    "router.utorrent.com:6881"];

#[derive(Clone)]
pub struct Node {
    addr: String,
    id: String,
}

#[derive(Clone)]
pub struct Announce {
    raw: BTreeMap<ByteString, Bencode>,
    from: net::SocketAddr,
    pub peer: net::SocketAddr,
    info_hash: Vec<u8>,
    pub info_hash_hex: String,
}


pub fn rand_bytes(n: i32) -> Vec<u8> {
    let mut result = Vec::new();
    for _ in 0..n {
        let t = random::<u8>();
        result.push(t);
    }
    return result;
}

type NodeID = Vec<u8>;


pub fn neighbour_id(target: NodeID, local: &NodeID) -> NodeID {
    let mut result = vec![0; 20];
    result[..10].copy_from_slice(&target[..10]);
    result[10..].copy_from_slice(&local[..10]);
    return result;
}


pub struct Query {
    t: String,
    y: String,
    q: String,
    a: BTreeMap<String, String>,
}

impl ToBencode for Query {
    fn to_bencode(&self) -> bencode::Bencode {
        let mut m = BTreeMap::new();
        m.insert(ByteString::from_str("t"), self.t.to_bencode());
        m.insert(ByteString::from_str("y"), self.y.to_bencode());
        m.insert(ByteString::from_str("q"), self.q.to_bencode());
        m.insert(ByteString::from_str("a"), self.a.to_bencode());
        Bencode::Dict(m)
    }
}

impl FromBencode for Query {
    type Err = String;

    fn from_bencode(bencode: &bencode::Bencode) -> Result<Query, String> {
        let mut q = Query {
            t: String::new(),
            y: String::new(),
            q: String::new(),
            a: BTreeMap::new(),
        };
        match bencode {
            &Bencode::Dict(ref m) => {
                if let Some(a) = m.get(&ByteString::from_str("a")) {
                    BTreeMap::from_bencode(a).map(|a| { q.a = a; });
                } else {
                    return Err("a not found".to_string());
                }
                if let Some(t) = m.get(&ByteString::from_str("t")) {
                    let _ = String::from_bencode(t).map(|t| { q.t = t; });
                } else {
                    return Err("t not found".to_string());
                }
                match m.get(&ByteString::from_str("y")) {
                    Some(y) => {
                        let _=String::from_bencode(y).map(|y| { q.y = y; });
                    }
                    None => return Err("y not found".to_string()),
                };
                match m.get(&ByteString::from_str("q")) {
                    Some(field_q) => {
                        q.q = String::from_bencode(field_q).unwrap_or_else(|e|{
                            println!("bencode to struct, q");
                            "".to_string()
                        });
                    }
                    None => return Err("q not found".to_string()),
                };
            }
            _ => return Err("not a dict".to_string()),
        }
        Ok(q)
    }
}

pub fn make_query<'a>(tid: String, q: String, a: &BTreeMap<String, String>) -> Query {
    Query {
        t: tid,
        y: String::from("q"),
        q,
        a: a.clone(),
    }
}

pub struct Reply {
    t: String,
    y: String,
    r: BTreeMap<String, String>,
}

impl ToBencode for Reply {
    fn to_bencode(&self) -> Bencode {
        let mut m = BTreeMap::new();
        m.insert(ByteString::from_str("t"), self.t.to_bencode());
        m.insert(ByteString::from_str("y"), self.y.to_bencode());
        m.insert(ByteString::from_str("a"), self.r.to_bencode());
        Bencode::Dict(m)
    }
}

pub fn make_reply(tid: String, r: &BTreeMap<String, String>) -> Reply {
    Reply {
        t: tid,
        y: "r".to_string(),
        r: r.clone(),
    }
}

pub fn decode_nodes(s: String) -> Vec<Node> {
    let mut nodes = vec![];
    let l = s.len();
    if l % 26 != 0 {
        return nodes;
    }

    let mut i = 0;
    while i < l {
        let chars: Vec<char> = s[i + 20..i + 24].chars().collect();
        let mut ip = String::new();
        ip.push(chars[0]);
        ip.push('.');
        ip.push(chars[1]);
        ip.push('.');
        ip.push(chars[2]);
        ip.push('.');
        ip.push(chars[3]);
        ip.push(':');
        let mut rdr = Cursor::new(&s[i + 24..i + 26]);
        let r = rdr.read_u16::<BigEndian>();
        match r {
            Ok(port) => ip.push_str((port as i32).to_string().as_ref()),
            Err(e) => continue,
        }
        let id = &s[i..20];
        nodes.push(Node { id: id.to_string(), addr: ip });

        i += 26;
    }
    nodes
}

pub struct RustDHT {
    node_last_send_time: u64,
    local_id: NodeID,
    conn: net::UdpSocket,
    mk_friends_pause_milli: u64,
    secret: String,
    bootstraps: Vec<String>,
}

impl RustDHT {
    pub fn max_friends_per_sec(mut self, mut n: i32) -> RustDHT {
        if n == 0 {
            n = 10;
        }
        if n > 1000 {
            n = 1000;
        }
        self.mk_friends_pause_milli = 1000 / n as u64;
        self
    }
    pub fn local_id(mut self, id: Vec<u8>) -> RustDHT {
        self.local_id = id;
        self
    }
    pub fn secret(mut self, s: String) -> RustDHT {
        self.secret = s;
        self
    }
    pub fn bootstraps(mut self, addr: Vec<String>) -> RustDHT {
        self.bootstraps = addr;
        self
    }
}

pub fn new_dht() -> RustDHT {
    let mut socket = match net::UdpSocket::bind("0.0.0.0:34254") {
        Ok(s) => s,
        Err(e) => panic!("couldn't bind socket: {}", e)
    };
    let mut result = RustDHT {
        node_last_send_time: 0,
        local_id: rand_bytes(20),
        conn: socket,
        mk_friends_pause_milli: 0,
        secret: String::from("IYHJFR%^&IO"),
        bootstraps: vec![],
    };
    for s in BOOTSTRAP_NODES.iter() {
        result.bootstraps.push(s.to_string());
    }
    return result;
}

impl RustDHT {
    pub fn start(self) -> (Vec<thread::JoinHandle<()>>, mpsc::Receiver<Announce>) {
        let (sender_node, rx_node) = mpsc::channel();
        let (sender_announce, rx_announce) = mpsc::channel();
        let arc_self = Arc::new(Mutex::new(self));
        let j = sender_node.clone();
        let tmp = arc_self.clone();
        let handle_join = thread::spawn(move || {
            let mut local: Vec<String> = vec![];
            {
                let d = tmp.lock().unwrap();
                for s in d.bootstraps.iter() {
                    local.push(s.to_string());
                }
            }
            for s in local {
                if let Err(e) = j.send(Node { addr: s.to_string(), id: vec2str(rand_bytes(20)) }){
                    println!("join:{}",e.to_string())
                }
            }
        });

        let tx_node = sender_node.clone();
        let tx_an = sender_announce.clone();
        let mut tmp = arc_self.clone();
        let handle_listen = thread::spawn(move || {
            loop {
                let mut buf: [u8; 2048] = [0; 2048];
                let mut local = tmp.lock().unwrap();
                match local.conn.recv_from(&mut buf) {
                    Ok((amt, src)) => local.on_message(buf[..amt].to_vec(), src, &tx_node, &tx_an),
                    Err(e) => continue,
                };
            }
        });

        let tmp = arc_self.clone();
        let handle_mk_friends = thread::spawn(move || {
            loop {
                if let Ok(n) = rx_node.recv() {
                    let local = tmp.lock().unwrap();
                    //self.find_node(n.addr, n.id.into_bytes());
                    let mut m = BTreeMap::new();
                    m.insert("id".to_string(), vec2str(neighbour_id(n.id.into_bytes(), &local.local_id)));
                    m.insert("target".to_string(), vec2str(rand_bytes(20)));
                    let q = make_query(vec2str(rand_bytes(2)), "find_node".to_string(), &m);

                    if let Ok(addr) = net::SocketAddrV4::from_str(n.addr.as_ref()) {
                        if let Ok(dat) = q.to_bencode().to_bytes() {
                            let result = local.conn.send_to(dat.as_ref(), net::SocketAddr::V4(addr));
                            if let Err(e)=result{
                                println!("make friends:{}",e.to_string())
                            }
                        }
                    }
                }
            }
        });
        let mut h = vec![];
        h.push(handle_join);
        h.push(handle_listen);
        h.push(handle_mk_friends);
        return (h, rx_announce);
    }

    fn on_message(&mut self, dat: Vec<u8>, addr: net::SocketAddr, tx_node: &mpsc::Sender<Node>, tx_announce: &mpsc::Sender<Announce>) {
        let ben = match bencode::from_vec(dat) {
            Ok(r) => r,
            _ => return,
        };
        let mut y_str = String::new();
        match ben {
            Bencode::Dict(ref m) => {
                if let Some(y) = m.get(&ByteString::from_str("y")) {
                    if let Ok(s) = String::from_bencode(y) {
                        y_str = s;
                    }
                }
                match y_str.as_ref() {
                    "q" => {
                        if let Some(q) = m.get(&ByteString::from_str("q")) {
                            if let Ok(s) = String::from_bencode(q) {
                                match s.as_ref() {
                                    "get_peers" => self.on_get_peers_query(&ben, addr),
                                    "announce_peer" => self.on_announce_peer_query(m, addr, tx_announce),
                                    _ => return,
                                }
                            }
                        }
                    }
                    "r" | "e" => {
                        if let Some(r_dict) = m.get(&ByteString::from_str("r")) {
                            if let Bencode::Dict(ref r) = r_dict {
                                if let Some(nodes_ben_str) = r.get(&ByteString::from_str("nodes")) {
                                    if let Ok(nodes_str) = String::from_bencode(nodes_ben_str) {
                                        let nodes = decode_nodes(nodes_str);
                                        for n in nodes.iter() {
                                            if self.node_last_send_time + self.mk_friends_pause_milli < get_now_millis() {
                                                continue;
                                            }
                                            self.node_last_send_time = get_now_millis();
                                            let _=tx_node.send(n.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => return,
                }
            }
            _ => return,
        }
    }

    fn gen_token(&self, from: net::SocketAddr) -> String {
        let mut h = sha1::Sha1::new();
        h.update(from.ip().to_string().as_bytes());
        h.update(self.secret.as_bytes());
        h.digest().to_string()
    }

    fn find_node(&self, to: String, target: NodeID) {
        let mut m = BTreeMap::new();
        m.insert("id".to_string(), vec2str(neighbour_id(target, &self.local_id)));
        m.insert("target".to_string(), vec2str(rand_bytes(20)));
        let q = make_query(vec2str(rand_bytes(2)), "find_node".to_string(), &m);

        if let Ok(addr) = net::SocketAddrV4::from_str(to.as_ref()) {
            if let Ok(dat) = q.to_bencode().to_bytes() {
                let _ = self.conn.send_to(dat.as_ref(), net::SocketAddr::V4(addr));
            }
        }
    }

    fn on_get_peers_query(&self, ben: &bencode::Bencode, from: net::SocketAddr) {
        let mut t_str = String::new();
        let mut id_str = String::new();
        if let bencode::Bencode::Dict(ref m) = ben {
            if let Some(t) = m.get(&ByteString::from_str("t")) {
                t_str = String::from_bencode(t).unwrap_or_default();
            } else {
                return;
            }
            if let Some(a) = m.get(&ByteString::from_str("a")) {
                if let bencode::Bencode::Dict(ref a_dict) = a {
                    if let Some(id) = a_dict.get(&ByteString::from_str("id")) {
                        id_str = String::from_bencode(id).unwrap_or_default();
                    }
                } else {
                    return;
                }
            } else {
                return;
            }
        } else {
            return;
        }
        let mut m = BTreeMap::new();
        m.insert("id".to_string(), vec2str(neighbour_id(id_str.into_bytes(), &self.local_id)));
        m.insert("nodes".to_string(), "".to_string());
        m.insert("token".to_string(), self.gen_token(from));
        let r = make_reply(t_str, &m);
        if let Ok(dat) = r.to_bencode().to_bytes() {
            let _ = self.conn.send_to(dat.as_ref(), from);
        }
    }

    fn is_token_available(&self, token: String, from: net::SocketAddr) -> bool {
        return self.gen_token(from).eq(&token);
    }

    fn on_announce_peer_query(&self, ben: &bencode::DictMap, from: net::SocketAddr, tx: &mpsc::Sender<Announce>) {
        if let Some(Bencode::Dict(ref a)) = ben.get(&ByteString::from_str("a")) {
            if let Some(Bencode::ByteString(token)) = a.get(&ByteString::from_str("token")) {
                if !self.is_token_available(vec2str(token.to_vec()), from) {
                    return;
                }
            } else {
                return;
            }
            let mut port = from.port();
            if let Some(Bencode::Number(implied)) = a.get(&ByteString::from_str("implied_port")) {
                if *implied == 0 {
                    if let Some(Bencode::Number(p)) = a.get(&ByteString::from_str("port")) {
                        port = *p as u16;
                    }
                }
            }
            if let Some(Bencode::ByteString(hash)) = a.get(&ByteString::from_str("info_hash")) {
                let a = Announce {
                    raw: ben.clone(),
                    from: from,
                    peer: net::SocketAddr::new(from.ip(), port),
                    info_hash: hash.to_vec(),
                    info_hash_hex: hex(hash.to_vec()),
                };
                let _ = tx.send(a);
            }
        }
    }
}

fn vec2str(dat: Vec<u8>) -> String {
    String::from_utf8(dat).unwrap_or_default()
}


fn get_now_millis() -> u64 {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    d.as_secs() * 1000 + d.subsec_nanos() as u64 / 1_000_000
}

static CHARS: &'static [u8] = b"0123456789abcdef";

pub fn hex(dat: Vec<u8>) -> String {
    let mut v = Vec::with_capacity(dat.len() * 2);
    for &byte in dat.iter() {
        v.push(CHARS[(byte >> 4) as usize]);
        v.push(CHARS[(byte & 0xf) as usize]);
    }

    unsafe {
        String::from_utf8_unchecked(v)
    }
}
