//! M3 — Signal session layer (ported from libsignal-go, Johnkhk fork).
//!
//! This slice: the persistent-session protobuf model from
//! `protocol/generated/v1/storage.pb.go` (`SessionStructure`,
//! `RecordStructure`, chains/chains-keys/message-keys/pending-pre-keys) plus
//! the session `Record` manager from `protocol/session/record.go`.
//!
//! Wire compatibility is locked by a golden test: the exact bytes produced by
//! libsignal-go's proto3 marshal for a full structure. Later slices (X3DH,
//! double ratchet, cipher, messages) build on this model.
//!
//! NOTE: proto3 semantics — default (zero/empty/nil) fields are omitted on
//! encode, which differs from our proto2-flavoured helpers in `wa2::pb`.

#![allow(dead_code)]

use crate::wa2::{pb, Error};

// ─────────────────────────────────────────────────────────────────────────────
// Structs (mirror `v1.SessionStructure_*` / `v1.RecordStructure`)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChainKey {
    pub index: u32,
    pub key: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MessageKey {
    pub index: u32,
    pub cipher_key: Vec<u8>,
    pub mac_key: Vec<u8>,
    pub iv: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chain {
    pub sender_ratchet_key: Vec<u8>,
    pub sender_ratchet_key_private: Vec<u8>,
    pub chain_key: Option<ChainKey>,
    pub message_keys: Vec<MessageKey>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingPreKey {
    pub pre_key_id: u32,
    pub signed_pre_key_id: u32,
    pub base_key: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionStructure {
    pub session_version: u32,
    pub local_identity_public: Vec<u8>,
    pub remote_identity_public: Vec<u8>,
    pub root_key: Vec<u8>,
    pub previous_counter: u32,
    pub sender_chain: Option<Chain>,
    pub receiver_chains: Vec<Chain>,
    pub pending_pre_key: Option<PendingPreKey>,
    pub remote_registration_id: u32,
    pub local_registration_id: u32,
    pub alice_base_key: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecordStructure {
    pub current_session: Option<SessionStructure>,
    pub previous_sessions: Vec<Vec<u8>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// proto3-style encode (omit defaults, ascending field order)
// ─────────────────────────────────────────────────────────────────────────────

fn en_var(out: &mut Vec<u8>, num: u64, v: u64) {
    if v == 0 {
        return;
    }
    pb::field_uint(num, v, out);
}

fn en_bytes(out: &mut Vec<u8>, num: u64, b: &[u8]) {
    if b.is_empty() {
        return;
    }
    pb::field_bytes(num, b, out);
}

fn en_msg(out: &mut Vec<u8>, num: u64, body: &[u8]) {
    if body.is_empty() {
        return;
    }
    pb::field_bytes(num, body, out);
}

impl ChainKey {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        en_var(&mut out, 1, self.index as u64);
        en_bytes(&mut out, 2, &self.key);
        out
    }
}

impl MessageKey {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        en_var(&mut out, 1, self.index as u64);
        en_bytes(&mut out, 2, &self.cipher_key);
        en_bytes(&mut out, 3, &self.mac_key);
        en_bytes(&mut out, 4, &self.iv);
        out
    }
}

impl Chain {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        en_bytes(&mut out, 1, &self.sender_ratchet_key);
        en_bytes(&mut out, 2, &self.sender_ratchet_key_private);
        if let Some(ck) = &self.chain_key {
            en_msg(&mut out, 3, &ck.encode());
        }
        for mk in &self.message_keys {
            pb::field_bytes(4, &mk.encode(), &mut out);
        }
        out
    }
}

impl PendingPreKey {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        en_var(&mut out, 1, self.pre_key_id as u64);
        en_bytes(&mut out, 2, &self.base_key);
        en_var(&mut out, 3, self.signed_pre_key_id as u64);
        out
    }
}

impl SessionStructure {
    /// proto3 marshal — defaults omitted.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        en_var(&mut out, 1, self.session_version as u64);
        en_bytes(&mut out, 2, &self.local_identity_public);
        en_bytes(&mut out, 3, &self.remote_identity_public);
        en_bytes(&mut out, 4, &self.root_key);
        en_var(&mut out, 5, self.previous_counter as u64);
        if let Some(sc) = &self.sender_chain {
            en_msg(&mut out, 6, &sc.encode());
        }
        for rc in &self.receiver_chains {
            pb::field_bytes(7, &rc.encode(), &mut out);
        }
        if let Some(ppk) = &self.pending_pre_key {
            en_msg(&mut out, 9, &ppk.encode());
        }
        en_var(&mut out, 10, self.remote_registration_id as u64);
        en_var(&mut out, 11, self.local_registration_id as u64);
        en_bytes(&mut out, 13, &self.alice_base_key);
        out
    }

    pub fn decode(data: &[u8]) -> Result<SessionStructure, Error> {
        let f = pb::parse(data)?;
        let mut s = SessionStructure::default();
        for x in &f {
            match (x.num, x.wire) {
                (1, 0) => s.session_version = x.v as u32,
                (2, 2) => s.local_identity_public = x.data.to_vec(),
                (3, 2) => s.remote_identity_public = x.data.to_vec(),
                (4, 2) => s.root_key = x.data.to_vec(),
                (5, 0) => s.previous_counter = x.v as u32,
                (6, 2) => s.sender_chain = Some(Chain::decode(x.data)?),
                (7, 2) => s.receiver_chains.push(Chain::decode(x.data)?),
                (9, 2) => s.pending_pre_key = Some(PendingPreKey::decode(x.data)?),
                (10, 0) => s.remote_registration_id = x.v as u32,
                (11, 0) => s.local_registration_id = x.v as u32,
                (13, 2) => s.alice_base_key = x.data.to_vec(),
                _ => {}
            }
        }
        Ok(s)
    }
}

impl Chain {
    fn decode(data: &[u8]) -> Result<Chain, Error> {
        let f = pb::parse(data)?;
        let mut c = Chain::default();
        for x in &f {
            match (x.num, x.wire) {
                (1, 2) => c.sender_ratchet_key = x.data.to_vec(),
                (2, 2) => c.sender_ratchet_key_private = x.data.to_vec(),
                (3, 2) => c.chain_key = Some(ChainKey::decode(x.data)?),
                (4, 2) => c.message_keys.push(MessageKey::decode(x.data)?),
                _ => {}
            }
        }
        Ok(c)
    }
}

impl ChainKey {
    fn decode(data: &[u8]) -> Result<ChainKey, Error> {
        let f = pb::parse(data)?;
        Ok(ChainKey {
            index: pb::find_var(&f, 1).unwrap_or_default() as u32,
            key: pb::find_bytes(&f, 2).unwrap_or_default().to_vec(),
        })
    }
}

impl MessageKey {
    fn decode(data: &[u8]) -> Result<MessageKey, Error> {
        let f = pb::parse(data)?;
        Ok(MessageKey {
            index: pb::find_var(&f, 1).unwrap_or_default() as u32,
            cipher_key: pb::find_bytes(&f, 2).unwrap_or_default().to_vec(),
            mac_key: pb::find_bytes(&f, 3).unwrap_or_default().to_vec(),
            iv: pb::find_bytes(&f, 4).unwrap_or_default().to_vec(),
        })
    }
}

impl PendingPreKey {
    fn decode(data: &[u8]) -> Result<PendingPreKey, Error> {
        let f = pb::parse(data)?;
        Ok(PendingPreKey {
            pre_key_id: pb::find_var(&f, 1).unwrap_or_default() as u32,
            signed_pre_key_id: pb::find_var(&f, 3).unwrap_or_default() as u32,
            base_key: pb::find_bytes(&f, 2).unwrap_or_default().to_vec(),
        })
    }
}

impl RecordStructure {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        if let Some(cs) = &self.current_session {
            pb::field_bytes(1, &cs.encode(), &mut out);
        }
        for ps in &self.previous_sessions {
            pb::field_bytes(2, ps, &mut out);
        }
        out
    }

    pub fn decode(data: &[u8]) -> Result<RecordStructure, Error> {
        let f = pb::parse(data)?;
        let mut r = RecordStructure::default();
        for x in &f {
            match (x.num, x.wire) {
                (1, 2) => r.current_session = Some(SessionStructure::decode(x.data)?),
                (2, 2) => r.previous_sessions.push(x.data.to_vec()),
                _ => {}
            }
        }
        Ok(r)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Session record manager (libsignal `protocol/session/record.go`)
// ─────────────────────────────────────────────────────────────────────────────

const MAX_ARCHIVED_STATES: usize = 40;

/// A record of a session's current and previous states.
pub struct SessionRecord {
    current: Option<SessionStructure>,
    previous: Vec<SessionStructure>,
}

impl SessionRecord {
    pub fn new(state: Option<SessionStructure>) -> SessionRecord {
        SessionRecord {
            current: state,
            previous: Vec::with_capacity(MAX_ARCHIVED_STATES),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<SessionRecord, Error> {
        let rec = RecordStructure::decode(bytes)?;
        Ok(SessionRecord {
            current: rec.current_session,
            previous: Vec::with_capacity(MAX_ARCHIVED_STATES),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        RecordStructure {
            current_session: self.current.clone(),
            previous_sessions: self.previous.iter().map(|s| s.encode()).collect(),
        }
        .encode()
    }

    pub fn state(&self) -> Option<&SessionStructure> {
        self.current.as_ref()
    }

    pub fn state_mut(&mut self) -> Option<&mut SessionStructure> {
        self.current.as_mut()
    }

    pub fn version(&self) -> Result<u32, Error> {
        self.current
            .as_ref()
            .map(|s| s.session_version)
            .ok_or(Error::Noise("no current session".into()))
    }

    pub fn alice_base_key(&self) -> Option<&[u8]> {
        self.current.as_ref().map(|s| s.alice_base_key.as_slice())
    }

    pub fn archive_current(&mut self) {
        if let Some(cur) = self.current.take() {
            if self.previous.len() >= MAX_ARCHIVED_STATES {
                self.previous.remove(0);
            }
            self.previous.push(cur);
        }
    }

    pub fn promote_state(&mut self, state: SessionStructure) {
        self.archive_current();
        self.current = Some(state);
    }

    /// Replace the current session state directly (no archiving).
    pub fn set_session_state(&mut self, state: SessionStructure) {
        self.current = Some(state);
    }

    /// Decoded copies of the archived states (oldest → newest).
    pub fn previous_states(&self) -> Vec<SessionStructure> {
        self.previous.clone()
    }

    /// Remove state at `idx` from the archive, then promote it (archiving the
    /// then-current state). Mirrors `Record.PromoteOldState`.
    pub fn promote_old_state(&mut self, idx: usize, state: SessionStructure) {
        if idx >= self.previous.len() {
            return;
        }
        self.previous.remove(idx);
        self.promote_state(state);
    }

    /// Check current + archived states for a matching session version and
    /// Alice base key. Like `Record.HasSessionState`, this does NOT promote.
    pub fn has_session_state(&self, version: u32, alice_base_key: &[u8]) -> bool {
        if let Some(cur) = &self.current {
            if cur.session_version == version && cur.alice_base_key.as_slice() == alice_base_key {
                return true;
            }
        }
        self.previous
            .iter()
            .any(|s| s.session_version == version && s.alice_base_key == alice_base_key)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden bytes produced by libsignal-go (cmd_session_golden) for the
    /// exact structure below — locks wire compatibility with the Go reference.
    fn fixture() -> SessionStructure {
        let chain_key = ChainKey {
            index: 12,
            key: b"chainkey1234567890chainkey1234567890x".to_vec(),
        };
        let msg_key = MessageKey {
            index: 34,
            cipher_key: b"cipherkey1234567890cipherkey123456789".to_vec(),
            mac_key: b"mackey1234567890mackey123456".to_vec(),
            iv: b"iviviviviviviviviviviviviviviviv".to_vec(),
        };
        SessionStructure {
            session_version: 3,
            local_identity_public: b"localidentpub1234567890localidentpub1234".to_vec(),
            remote_identity_public: b"remoteidentpub1234567890remoteidentpub12".to_vec(),
            root_key: b"rootkey1234567890rootkey1234567890roo".to_vec(),
            previous_counter: 5,
            sender_chain: Some(Chain {
                sender_ratchet_key: b"ratchet1234567890ratchet12345678901".to_vec(),
                sender_ratchet_key_private: b"ratchet1234567890ratchet1234567890222".to_vec(),
                chain_key: Some(chain_key.clone()),
                message_keys: vec![msg_key.clone()],
            }),
            receiver_chains: vec![Chain {
                sender_ratchet_key: b"rcvratchet1234567890rcvratchet12".to_vec(),
                chain_key: Some(chain_key),
                message_keys: vec![msg_key],
                ..Default::default()
            }],
            pending_pre_key: Some(PendingPreKey {
                pre_key_id: 77,
                signed_pre_key_id: 88,
                base_key: b"pendingbasekey1234567890pendingbaseke".to_vec(),
            }),
            remote_registration_id: 222,
            local_registration_id: 333,
            alice_base_key: b"alicebasekey1234567890alicebasekey1234".to_vec(),
        }
    }

    #[test]
    fn session_structure_marshals_exactly_like_libsignal() {
        let s = fixture();
        let got = s.encode();
        let want = "080312286c6f63616c6964656e74707562313233343536373839306c6f63616c6964656e74707562313233341a2872656d6f74656964656e747075623132333435363738393072656d6f74656964656e7470756231322225726f6f746b657931323334353637383930726f6f746b657931323334353637383930726f6f280532e2010a2372617463686574313233343536373839307261746368657431323334353637383930311225726174636865743132333435363738393072617463686574313233343536373839303232321a29080c1225636861696e6b657931323334353637383930636861696e6b657931323334353637383930782269082212256369706865726b6579313233343536373839306369706865726b65793132333435363738391a1c6d61636b6579313233343536373839306d61636b6579313233343536222069766976697669766976697669766976697669766976697669766976697669763ab8010a2072637672617463686574313233343536373839307263767261746368657431321a29080c1225636861696e6b657931323334353637383930636861696e6b657931323334353637383930782269082212256369706865726b6579313233343536373839306369706865726b65793132333435363738391a1c6d61636b6579313233343536373839306d61636b6579313233343536222069766976697669766976697669766976697669766976697669766976697669764a2b084d122570656e64696e67626173656b65793132333435363738393070656e64696e67626173656b65185850de0158cd026a26616c696365626173656b657931323334353637383930616c696365626173656b657931323334";
        assert_eq!(hex(&got), want);
    }

    #[test]
    fn record_marshals_exactly_like_libsignal() {
        let rec = RecordStructure {
            current_session: Some(fixture()),
            previous_sessions: vec![b"prev1".to_vec(), b"prev2".to_vec()],
        };
        let got = rec.encode();
        let want = "0afa04080312286c6f63616c6964656e74707562313233343536373839306c6f63616c6964656e74707562313233341a2872656d6f74656964656e747075623132333435363738393072656d6f74656964656e7470756231322225726f6f746b657931323334353637383930726f6f746b657931323334353637383930726f6f280532e2010a2372617463686574313233343536373839307261746368657431323334353637383930311225726174636865743132333435363738393072617463686574313233343536373839303232321a29080c1225636861696e6b657931323334353637383930636861696e6b657931323334353637383930782269082212256369706865726b6579313233343536373839306369706865726b65793132333435363738391a1c6d61636b6579313233343536373839306d61636b6579313233343536222069766976697669766976697669766976697669766976697669766976697669763ab8010a2072637672617463686574313233343536373839307263767261746368657431321a29080c1225636861696e6b657931323334353637383930636861696e6b657931323334353637383930782269082212256369706865726b6579313233343536373839306369706865726b65793132333435363738391a1c6d61636b6579313233343536373839306d61636b6579313233343536222069766976697669766976697669766976697669766976697669766976697669764a2b084d122570656e64696e67626173656b65793132333435363738393070656e64696e67626173656b65185850de0158cd026a26616c696365626173656b657931323334353637383930616c696365626173656b6579313233341205707265763112057072657632";
        assert_eq!(hex(&got), want);
    }

    #[test]
    fn session_roundtrip() {
        // All fields set — encode→decode must reproduce every value.
        let rec = RecordStructure {
            current_session: Some(fixture()),
            previous_sessions: vec![b"prev1".to_vec(), b"prev2".to_vec()],
        };
        let decoded = RecordStructure::decode(&rec.encode()).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn session_omits_defaults_roundtrip() {
        // proto3 omits default fields; a default-only structure encodes to zero
        // bytes and decodes back to defaults.
        let empty = SessionStructure::default();
        assert_eq!(empty.encode(), Vec::<u8>::new());
        assert_eq!(SessionStructure::decode(&[]).unwrap(), empty);
    }

    #[test]
    fn record_archives_and_promotes() {
        let mut rec = SessionRecord::new(Some(fixture()));
        let version = rec.version().unwrap();
        assert_eq!(version, 3);

        let mut archived = fixture();
        archived.session_version = 4;
        archived.alice_base_key = b"otheralicebasekey1234567890otheraliceba".to_vec();
        rec.promote_state(archived.clone());
        assert_eq!(rec.version().unwrap(), 4);
        assert_eq!(rec.state().unwrap().alice_base_key, archived.alice_base_key);

        // The v3 state is in the archive. HasSessionState finds it without
        // promoting; promote_old_state then swaps it back to current.
        assert!(rec.has_session_state(3, b"alicebasekey1234567890alicebasekey1234"));
        assert_eq!(rec.version().unwrap(), 4);
        let old_idx = rec
            .previous_states()
            .iter()
            .position(|s| s.session_version == 3)
            .unwrap();
        let old_state = rec.previous_states()[old_idx].clone();
        rec.promote_old_state(old_idx, old_state);
        assert_eq!(rec.version().unwrap(), 3);

        // Serialize the whole record and pull it back.
        let bytes = rec.to_bytes();
        let reloaded = SessionRecord::from_bytes(&bytes).unwrap();
        // After promote_old_state the archived v3 fixture is current again;
        // serialization round-trips it.
        assert_eq!(reloaded.state().map(|s| s.session_version), Some(3));
        assert_eq!(
            reloaded.state().map(|s| s.alice_base_key.as_slice()),
            Some(&b"alicebasekey1234567890alicebasekey1234"[..])
        );
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
}