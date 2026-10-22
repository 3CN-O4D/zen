//! Resource table for native OS resources (processes, threads) owned by the VM.
//! This is a pointer-free handle table: Zen scripts receive integer IDs.

use std::io::BufReader;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::process::{Child, ChildStdin, ChildStdout};
use std::thread::JoinHandle;

use crate::value::Value;

/// Represents an active native OS resource owned by the VM.
pub enum Resource {
    Process(Child, Option<BufReader<ChildStdout>>, Option<ChildStdin>),
    Thread(JoinHandle<Result<Value, String>>),
    TcpStream(TcpStream),
    TcpListener(TcpListener),
    UdpSocket(UdpSocket),
    RawSocket(i32),
}

impl std::fmt::Debug for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resource::Process(_, _, _) => f.debug_tuple("Process").finish(),
            Resource::Thread(_) => f.debug_tuple("Thread").finish(),
            Resource::TcpStream(_) => f.debug_tuple("TcpStream").finish(),
            Resource::TcpListener(_) => f.debug_tuple("TcpListener").finish(),
            Resource::UdpSocket(_) => f.debug_tuple("UdpSocket").finish(),
            Resource::RawSocket(_) => f.debug_tuple("RawSocket").finish(),
        }
    }
}

/// A safe, pointer-free handle table for native resources.
#[derive(Debug)]
pub struct ResourceTable {
    items: Vec<Option<Resource>>,
}

impl ResourceTable {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Takes ownership of a native resource and returns a safe integer ID.
    pub fn insert(&mut self, resource: Resource) -> usize {
        // Look for an empty slot first to prevent infinite Vec growth.
        for (index, slot) in self.items.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(resource);
                return index;
            }
        }
        let id = self.items.len();
        self.items.push(Some(resource));
        id
    }

    /// Safely retrieves a mutable reference to a resource.
    pub fn get_mut(&mut self, id: usize) -> Option<&mut Resource> {
        self.items.get_mut(id)?.as_mut()
    }

    /// Removes a resource from the table and returns it (e.g., for waiting/joining).
    pub fn take(&mut self, id: usize) -> Option<Resource> {
        if id < self.items.len() {
            self.items[id].take()
        } else {
            None
        }
    }

    /// Returns the number of allocated resource slots.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` when no resource slots are allocated.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for ResourceTable {
    fn default() -> Self {
        Self::new()
    }
}
