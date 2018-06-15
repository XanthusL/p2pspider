extern crate rand;

use rand::prelude::*;

mod lib;

fn main() {
    println!("hello world");
}

fn run() {
    let mut d = lib::dht::new_dht();
    d = d
        .local_id("0.0.0.0".as_bytes().to_vec())
        .max_friends_per_sec(50)
        .secret("tmp-secret".to_string());
    let (handles, rx) = d.start();
    loop {
        match rx.recv() {
            Ok(announce) => {
                // todo
                // if is_exist(announce.info_hash_hex){continue}
                // if in_block_list(announce.info_hash_hex){continue}
                let mut peer = lib::wire::new(announce.info_hash_hex, announce.peer.to_string()).unwrap();
                let data = peer.fetch().unwrap_or_else(|e| {
                    // todo add announce.peer to block list
                    vec![]
                });
                // todo save data to announce.info_hash_hex.torrent
                // benCode{"info":data}
                // todo parse data
            }
            Err(_) => continue,
        }
    }
}
/*
https://github.com/fanpei91/p2pspider

type torrent struct {
	infohashHex string
	name        string
	length      int64
	files       []*tfile
}

func (t *torrent) String() string {
	return fmt.Sprintf(
		"link: %s\nname: %s\nsize: %d\nfile: %d\n",
		fmt.Sprintf("magnet:?xt=urn:btih:%s", t.infohashHex),
		t.name,
		t.length,
		len(t.files),
	)
}

func newTorrent(meta []byte, infohashHex string) (*torrent, error) {
	dict, err := bencode.Decode(bytes.NewBuffer(meta))
	if err != nil {
		return nil, err
	}
	t := &torrent{infohashHex: infohashHex}
	if name, ok := dict["name.utf-8"].(string); ok {
		t.name = name
	} else if name, ok := dict["name"].(string); ok {
		t.name = name
	}
	if length, ok := dict["length"].(int64); ok {
		t.length = length
	}
	var total int64
	if files, ok := dict["files"].([]interface{}); ok {
		for _, file := range files {
			var filename string
			var filelength int64
			if f, ok := file.(map[string]interface{}); ok {
				if inter, ok := f["path.utf-8"].([]interface{}); ok {
					path := make([]string, len(inter))
					for i, v := range inter {
						path[i] = fmt.Sprint(v)
					}
					filename = strings.Join(path, "/")
				} else if inter, ok := f["path"].([]interface{}); ok {
					path := make([]string, len(inter))
					for i, v := range inter {
						path[i] = fmt.Sprint(v)
					}
					filename = strings.Join(path, "/")
				}
				if length, ok := f["length"].(int64); ok {
					filelength = length
					total += filelength
				}
				t.files = append(t.files, &tfile{name: filename, length: filelength})
			}
		}
	}
	if t.length == 0 {
		t.length = total
	}
	if len(t.files) == 0 {
		t.files = append(t.files, &tfile{name: t.name, length: t.length})
	}
	return t, nil
}
*/