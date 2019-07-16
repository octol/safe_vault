// Copyright 2019 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{rpc::Rpc, vault::Init, Result};
use bincode;
use log::{error, trace};
use pickledb::{PickleDb, PickleDbDumpPolicy};
use rand::{distributions::Standard, thread_rng, Rng};
use safe_nd::{ClientPublicId, PublicId, PublicKey, Request, XorName};
use serde::Serialize;
use std::{fs, path::Path};
use unwrap::unwrap;

pub(crate) fn new_db<D: AsRef<Path>, N: AsRef<Path>>(
    db_dir: D,
    db_name: N,
    init_mode: Init,
) -> Result<PickleDb> {
    let db_path = db_dir.as_ref().join(db_name);
    if init_mode == Init::New {
        trace!("Creating database at {}", db_path.display());
        fs::create_dir_all(db_dir)?;
        let mut db = PickleDb::new_bin(db_path, PickleDbDumpPolicy::AutoDump);
        // Write then delete a value to ensure DB file is actually written to disk.
        db.set("", &"")?;
        let _ = db.rem("")?;
        return Ok(db);
    }
    trace!("Loading database at {}", db_path.display());
    let result = PickleDb::load_bin(db_path.clone(), PickleDbDumpPolicy::AutoDump);
    if let Err(ref error) = &result {
        error!("Failed to load {}: {}", db_path.display(), error);
    }
    Ok(result?)
}

pub(crate) fn random_vec(size: usize) -> Vec<u8> {
    thread_rng().sample_iter(&Standard).take(size).collect()
}

pub(crate) fn serialise<T: Serialize>(data: &T) -> Vec<u8> {
    unwrap!(bincode::serialize(data))
}

pub(crate) fn owner(public_id: &PublicId) -> Option<&ClientPublicId> {
    match public_id {
        PublicId::Node(_) => None,
        PublicId::Client(pub_id) => Some(pub_id),
        PublicId::App(pub_id) => Some(pub_id.owner()),
    }
}

pub(crate) fn own_key(public_id: &PublicId) -> Option<&PublicKey> {
    match public_id {
        PublicId::Node(_) => None,
        PublicId::Client(ref client) => Some(client.public_key()),
        PublicId::App(ref app) => Some(app.public_key()),
    }
}

pub(crate) fn rpc_elder_address(rpc: &Rpc) -> Option<XorName> {
    match rpc {
        Rpc::Request { ref requester, .. } | Rpc::Response { ref requester, .. } => {
            let client_pk = own_key(&requester)?;
            Some(XorName::from(*client_pk))
        }
    }
}

pub(crate) fn dst_elders_address(request: &Request) -> Option<&XorName> {
    use Request::*;
    match request {
        PutIData(ref data) => Some(data.name()),
        GetIData(ref address) => Some(address.name()),
        DeleteUnpubIData(ref address) => Some(address.name()),
        PutMData(ref data) => Some(data.name()),
        GetMData(ref address)
        | GetMDataValue { ref address, .. }
        | DeleteMData(ref address)
        | GetMDataShell(ref address)
        | GetMDataVersion(ref address)
        | ListMDataEntries(ref address)
        | ListMDataKeys(ref address)
        | ListMDataValues(ref address)
        | SetMDataUserPermissions { ref address, .. }
        | DelMDataUserPermissions { ref address, .. }
        | ListMDataPermissions(ref address)
        | ListMDataUserPermissions { ref address, .. }
        | MutateSeqMDataEntries { ref address, .. }
        | MutateUnseqMDataEntries { ref address, .. } => Some(address.name()),
        PutAData(ref data) => Some(data.name()),
        GetAData(ref address)
        | GetADataShell { ref address, .. }
        | DeleteAData(ref address)
        | GetADataRange { ref address, .. }
        | GetADataIndices(ref address)
        | GetADataLastEntry(ref address)
        | GetADataPermissions { ref address, .. }
        | GetPubADataUserPermissions { ref address, .. }
        | GetUnpubADataUserPermissions { ref address, .. }
        | GetADataOwners { ref address, .. }
        | AddPubADataPermissions { ref address, .. }
        | AddUnpubADataPermissions { ref address, .. }
        | SetADataOwner { ref address, .. } => Some(address.name()),
        AppendSeq { ref append, .. } | AppendUnseq(ref append) => Some(append.address.name()),
        TransferCoins {
            ref destination, ..
        } => Some(destination),
        CreateBalance {
            // ref new_balance_owner,
            ..
        } => None, // Some(XorName::from(new_balance_owner)),
        CreateLoginPacket(account_data) => Some(account_data.destination()),
        CreateLoginPacketFor {
            new_login_packet,
            ..
        } => Some(new_login_packet.destination()),
        UpdateLoginPacket(account_data) => Some(account_data.destination()),
        GetLoginPacket(ref name) => Some(name),
        GetBalance
        | ListAuthKeysAndVersion
        | InsAuthKey { .. }
        | DelAuthKey { .. } => None,
    }
}

/// Temporary helpers to work around changes required in safe-nd.
pub(crate) mod work_arounds {
    use safe_nd::ADataKind;

    pub fn is_adata_kind_pub(kind: ADataKind) -> bool {
        match kind {
            ADataKind::PubSeq | ADataKind::PubUnseq => true,
            ADataKind::UnpubSeq | ADataKind::UnpubUnseq => false,
        }
    }
}
