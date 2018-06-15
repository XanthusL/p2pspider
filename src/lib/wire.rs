extern crate bencode;
extern crate byteorder;
extern crate sha1;

use self::bencode::{Bencode, FromBencode, ToBencode};
use self::bencode::DictMap;
use self::bencode::util::ByteString;
use self::byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std;
use std::collections::BTreeMap;
use std::io::Read;
use std::io::Write;
use std::net;
use std::time;

const PER_BLOCK: i32 = 16384;
const MAX_META_DATA_SIZE: i32 = PER_BLOCK * 1024;
const EXTENDED: u8 = 20;
const EXT_HANDSHAKE: u8 = 0;

const ERR_EXT_HEADER: &str = "invalid extention header response";
const ERR_INVALID_PIECE: &str = "invalid piece response";


fn random_peer_id() -> String {
    let b = super::dht::rand_bytes(20);
    super::dht::hex(b)
//	b := make([]byte, 20)
//	rand.Read(b)
//	return string(b)
}

pub struct Wire {
    info_hash: String,
    peer_id: String,
    from: String,
    conn: net::TcpStream,
    timeout_sec: i32,
    metadata_size: i32,
    ut_metadata: i32,
    num_of_pieces: i32,
    pieces: Vec<Vec<u8>>,
    err: String,
}

struct Meta {
    data: Vec<u8>,
    err: String,
}

pub fn new(info: String, from: String) -> Result<Wire, std::io::Error> {
    let mut stream = net::TcpStream::connect(from.as_str())?;
    Ok(Wire {
        info_hash: info,
        peer_id: random_peer_id(),
        from: from,
        conn: stream,
        timeout_sec: 5,
        metadata_size: 0,
        ut_metadata: 0,
        num_of_pieces: 0,
        pieces: Vec::new(),
        err: String::new(),
    })
}

impl Wire {
    pub fn fetch(&mut self) -> Result<Vec<u8>, String> {
        let _ = self.conn.set_read_timeout(Some(time::Duration::from_secs(self.timeout_sec as u64)));
        //w.handshake(ctx)
        let mut h = self.pre_header();
        h.append(&mut self.info_hash.clone().into_bytes());
        h.append(&mut self.peer_id.clone().into_bytes());
        self.conn.write(&h).or_else(|e| { Err(e.to_string()) })?;
        //w.onHandshake(ctx)
        self.on_handshake()?;
        //w.extHandshake(ctx)
        self.ext_handshake()?;
        loop {
            let data = self.next()?;
            if data[0] != EXTENDED {
                continue;
            }
            self.on_extended(data[1], data[2..].to_vec())?;
            if !self.is_done() {
                continue;
            }
            let m = self.pieces.concat();
            let mut h = sha1::Sha1::new();
            h.update(&m[..]);
            if h.digest().to_string() == self.info_hash {
                return Ok(m);
            }
            return Err("metadata checksum mismatch".to_string());
        }
    }

    pub fn close(&self) {
        let _ = self.conn.try_clone();
    }

    fn pre_header(&self) -> Vec<u8> {
        let mut r = "BitTorrent protocol".as_bytes().to_vec();
        r.insert(0, 19);
        r.append(&mut vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x01]);
        return r;
    }

    fn is_done(&self) -> bool {
        for i in self.pieces.iter() {
            if i.len() == 0 {
                return false;
            }
        }
        return true;
    }
    fn on_handshake(&mut self) -> Result<(), String> {
        let mut buf = [0; 68];
        self.conn.read(&mut buf[..]).or_else(|e| { Err(e.to_string()) })?;
        if buf[..20] != self.pre_header()[..20] {
            return Err("remote peer not supporting bittorrent protocol".to_string());
        }

        if buf[25] & 0x10 != 0x10 {
            return Err("remote peer not supporting extention protocol".to_string());
        }
        if buf[28..48].to_vec() != self.info_hash.clone().into_bytes() {
            return Err("invalid bittorrent header response".to_string());
        }
        Ok(())
    }

    fn ext_handshake(&mut self) -> Result<(), String> {
        let mut v = vec![EXTENDED, EXT_HANDSHAKE];
        let mut m = DictMap::new();
        let mut m_inner = DictMap::new();
        m_inner.insert(ByteString::from_str("ut_metadata"), Bencode::Number(1));
        m.insert(ByteString::from_str("m"), Bencode::Dict(m_inner));
        let mut dat = Bencode::Dict(m).to_bytes().or_else(|e| { Err(e.to_string()) })?;
        v.append(&mut dat);
        self.conn.write(&v).or_else(|e| { Err(e.to_string()) })?;
        Ok(())
    }

    fn next(&mut self) -> Result<Vec<u8>, String> {
        let size = self.conn.read_u32::<BigEndian>().or_else(|e| { Err(e.to_string()) })?;
        let mut data = Vec::new();
        unsafe { data.set_len(size as usize); };
        self.conn.read(&mut data[..]).or_else(|e| { Err(e.to_string()) })?;
        Ok(data.to_vec())
    }

    fn on_extended(&mut self, ext: u8, payload: Vec<u8>) -> Result<(), String> {
        if ext == 0 {
            self.on_ext_handshake(payload)?;
        } else {
            let (piece, index) = self.on_piece(payload)?;
            self.pieces[index as usize] = piece;
        }
        Ok(())
    }
    fn on_piece(&self, payload: Vec<u8>) -> Result<(Vec<u8>, i32), String> {
        let l = payload.len();
        let mut trailer_index: isize = -1;
        for (i, b) in payload.iter().enumerate() {
            if *b == b'e' && i < l - 1 && payload[i + 1] == b'e' {
                trailer_index = i as isize;
            }
        }
        if trailer_index == -1 {
            return Err(ERR_INVALID_PIECE.to_string());
        }
        trailer_index += 2;
        let mut p_index: i32 = 0;
        let ben = bencode::from_vec(payload[..trailer_index as usize].to_vec()).or_else(|e| { Err(e.msg) })?;
        if let Bencode::Dict(ref m) = ben {
            if let Some(Bencode::Number(piece_index)) = m.get(&ByteString::from_str("piece")) {
                p_index = *piece_index as i32;
                if p_index > self.num_of_pieces {
                    return Err(ERR_INVALID_PIECE.to_string());
                }
            } else {
                return Err(ERR_INVALID_PIECE.to_string());
            }
            if let Some(Bencode::Number(t)) = m.get(&ByteString::from_str("msg_type")) {
                if *t != 1 {
                    return Err(ERR_INVALID_PIECE.to_string());
                }
            } else {
                return Err(ERR_INVALID_PIECE.to_string());
            }
        }
        Ok((payload[trailer_index as usize..].to_vec(), p_index))
    }


    fn request_pieces(&mut self, i: i32) {
        let mut dat = vec![EXTENDED, self.ut_metadata as u8];
        let mut m = DictMap::new();
        m.insert(ByteString::from_str("msg_type"), Bencode::Number(0));
        m.insert(ByteString::from_str("piece"), Bencode::Number(i as i64));
        let mut b = Bencode::Dict(m).to_bytes().unwrap_or_default();
        dat.append(&mut b);
        let _ = self.conn.write_u32::<BigEndian>(dat.len() as u32);
    }

    fn on_ext_handshake(&mut self, payload: Vec<u8>) -> Result<(), String> {
        let ben: Bencode = bencode::from_vec(payload).or_else(|e| { Err(e.msg) })?;
        let mut meta_size: i32 = 0;
        let mut ut_meta: i32 = 0;
        if let Bencode::Dict(ref m) = ben {
            if let Some(Bencode::Number(size)) = m.get(&ByteString::from_str("metadata_size")) {
                meta_size = *size as i32;
                if meta_size > MAX_META_DATA_SIZE {
                    return Err("metadata_size too long".to_string());
                }
            } else {
                return Err(ERR_EXT_HEADER.to_string());
            }
            if let Some(Bencode::Dict(ref inner_m)) = m.get(&ByteString::from_str("m")) {
                if let Some(Bencode::Number(u)) = m.get(&ByteString::from_str("ut_metadata")) {
                    ut_meta = *u as i32;
                } else {
                    return Err(ERR_EXT_HEADER.to_string());
                }
            } else {
                return Err(ERR_EXT_HEADER.to_string());
            }
        }
        self.metadata_size = meta_size;
        self.ut_metadata = ut_meta;
        self.num_of_pieces = (meta_size + PER_BLOCK - 1) / PER_BLOCK;
        self.pieces = Vec::new();
        for i in 0..self.num_of_pieces {
            self.pieces.push(vec![]);
            self.request_pieces(i);
        }
        Ok(())
    }
}
