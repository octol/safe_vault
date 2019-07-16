// Copyright 2019 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

// TODO: make these tests work without mock too.
#![cfg(feature = "mock")]
#![forbid(
    exceeding_bitshifts,
    mutable_transmutes,
    no_mangle_const_items,
    unknown_crate_types,
    warnings
)]
#![deny(
    bad_style,
    deprecated,
    improper_ctypes,
    missing_docs,
    non_shorthand_field_patterns,
    overflowing_literals,
    plugin_as_library,
    stable_features,
    unconditional_recursion,
    unknown_lints,
    unsafe_code,
    unused,
    unused_allocation,
    unused_attributes,
    unused_comparisons,
    unused_features,
    unused_parens,
    while_true
)]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results
)]
#![allow(
    box_pointers,
    missing_copy_implementations,
    missing_debug_implementations,
    variant_size_differences
)]

#[macro_use]
mod common;

use self::common::{Environment, TestClient, TestVault};
use rand::Rng;
use safe_nd::{
    AData, Coins, Error as NdError, IData, IDataAddress, LoginPacket, PubImmutableData,
    PubSeqAppendOnlyData, PubUnseqAppendOnlyData, Request, Response, SeqAppendOnly,
    UnpubImmutableData, UnpubSeqAppendOnlyData, UnpubUnseqAppendOnlyData, UnseqAppendOnly, XorName,
};
use safe_vault::COST_OF_PUT;
use unwrap::unwrap;

#[test]
fn client_connects() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();
    let mut client = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client, &mut vault);
}

#[test]
fn login_packets() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();
    let conn_info = vault.connection_info();

    let mut client = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client, &mut vault);

    let login_packet_data = vec![0; 32];
    let login_packet_locator: XorName = env.rng().gen();

    // Try to get a login packet that does not exist yet.
    let message_id = client.send_request(
        conn_info.clone(),
        Request::GetLoginPacket(login_packet_locator),
    );
    env.poll(&mut vault);

    match client.expect_response(message_id) {
        Response::GetLoginPacket(Err(NdError::NoSuchLoginPacket)) => (),
        x => unexpected!(x),
    }

    // Create a new login packet.
    let login_packet = unwrap!(LoginPacket::new(
        login_packet_locator,
        *client.public_id().public_key(),
        login_packet_data.clone(),
        client.full_id().sign(&login_packet_data),
    ));

    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::CreateLoginPacket(login_packet.clone()),
    );

    // Try to get the login packet data and signature.
    let message_id = client.send_request(
        conn_info.clone(),
        Request::GetLoginPacket(login_packet_locator),
    );
    env.poll(&mut vault);

    match client.expect_response(message_id) {
        Response::GetLoginPacket(Ok((data, sig))) => {
            assert_eq!(data, login_packet_data);

            match client.public_id().public_key().verify(&sig, &data) {
                Ok(()) => (),
                x => unexpected!(x),
            }
        }
        x => unexpected!(x),
    }

    // Putting login packet to the same address should fail.
    let message_id =
        client.send_request(conn_info.clone(), Request::CreateLoginPacket(login_packet));
    env.poll(&mut vault);

    match client.expect_response(message_id) {
        Response::Mutation(Err(NdError::LoginPacketExists)) => (),
        x => unexpected!(x),
    }

    // Getting login packet from non-owning client should fail.
    {
        let mut client = TestClient::new(env.rng());
        common::establish_connection(&mut env, &mut client, &mut vault);

        let message_id = client.send_request(
            conn_info.clone(),
            Request::GetLoginPacket(login_packet_locator),
        );
        env.poll(&mut vault);

        match client.expect_response(message_id) {
            Response::GetLoginPacket(Err(NdError::AccessDenied)) => (),
            x => unexpected!(x),
        }
    }
}

#[test]
fn update_login_packet() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();
    let conn_info = vault.connection_info();

    let mut client = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client, &mut vault);

    let login_packet_data = vec![0; 32];
    let login_packet_locator: XorName = env.rng().gen();

    // Create a new login packet.
    let login_packet = unwrap!(LoginPacket::new(
        login_packet_locator,
        *client.public_id().public_key(),
        login_packet_data.clone(),
        client.full_id().sign(&login_packet_data),
    ));

    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::CreateLoginPacket(login_packet.clone()),
    );

    // Update the login packet data.
    let new_login_packet_data = vec![1; 32];
    let client_public_key = *client.public_id().public_key();
    let signature = client.full_id().sign(&new_login_packet_data);
    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::UpdateLoginPacket(unwrap!(LoginPacket::new(
            login_packet_locator,
            client_public_key,
            new_login_packet_data.clone(),
            signature,
        ))),
    );

    // Try to get the login packet data and signature.
    let message_id = client.send_request(
        conn_info.clone(),
        Request::GetLoginPacket(login_packet_locator),
    );
    env.poll(&mut vault);

    match client.expect_response(message_id) {
        Response::GetLoginPacket(Ok((data, sig))) => {
            assert_eq!(data, new_login_packet_data);
            unwrap!(client.public_id().public_key().verify(&sig, &data));
        }
        x => unexpected!(x),
    }

    // Updating login packet from non-owning client should fail.
    {
        let mut client = TestClient::new(env.rng());
        common::establish_connection(&mut env, &mut client, &mut vault);

        let message_id = client.send_request(
            conn_info.clone(),
            Request::UpdateLoginPacket(login_packet.clone()),
        );
        env.poll(&mut vault);

        match client.expect_response(message_id) {
            Response::Mutation(Err(NdError::AccessDenied)) => (),
            x => unexpected!(x),
        }
    }
}

#[test]
fn coin_operations() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();
    let conn_info = vault.connection_info();

    let mut client_a = TestClient::new(env.rng());
    let mut client_b = TestClient::new(env.rng());

    common::establish_connection(&mut env, &mut client_a, &mut vault);
    common::establish_connection(&mut env, &mut client_b, &mut vault);

    let balance = common::get_balance(&mut env, &mut client_a, &mut vault);
    assert_eq!(balance, unwrap!(Coins::from_nano(0)));

    // Create A's balance
    let public_key = *client_a.public_id().public_key();
    let message_id = client_a.send_request(
        conn_info.clone(),
        Request::CreateBalance {
            new_balance_owner: public_key,
            amount: unwrap!(Coins::from_nano(10)),
            transaction_id: 0,
        },
    );
    env.poll(&mut vault);

    match client_a.expect_response(message_id) {
        Response::Transaction(Ok(transaction)) => {
            assert_eq!(transaction.id, 0);
            assert_eq!(transaction.amount, unwrap!(Coins::from_nano(10)))
        }
        x => unexpected!(x),
    }

    let balance = common::get_balance(&mut env, &mut client_a, &mut vault);
    assert_eq!(balance, unwrap!(Coins::from_nano(10)));

    // Create B's balance
    let message_id = client_a.send_request(
        conn_info.clone(),
        Request::CreateBalance {
            new_balance_owner: *client_b.public_id().public_key(),
            amount: unwrap!(Coins::from_nano(1)),
            transaction_id: 0,
        },
    );
    env.poll(&mut vault);

    match client_a.expect_response(message_id) {
        Response::Transaction(Ok(transaction)) => {
            assert_eq!(transaction.id, 0);
            assert_eq!(transaction.amount, unwrap!(Coins::from_nano(1)))
        }
        x => unexpected!(x),
    }

    let balance_a = common::get_balance(&mut env, &mut client_a, &mut vault);
    let balance_b = common::get_balance(&mut env, &mut client_b, &mut vault);
    assert_eq!(balance_a, unwrap!(Coins::from_nano(9)));
    assert_eq!(balance_b, unwrap!(Coins::from_nano(1)));

    // Transfer coins from A to B
    let message_id = client_a.send_request(
        conn_info,
        Request::TransferCoins {
            destination: *client_b.public_id().name(),
            amount: unwrap!(Coins::from_nano(2)),
            transaction_id: 1,
        },
    );
    env.poll(&mut vault);

    match client_a.expect_response(message_id) {
        Response::Transaction(Ok(transaction)) => {
            assert_eq!(transaction.id, 1);
            assert_eq!(transaction.amount, unwrap!(Coins::from_nano(2)))
        }
        x => unexpected!(x),
    }

    let balance_a = common::get_balance(&mut env, &mut client_a, &mut vault);
    let balance_b = common::get_balance(&mut env, &mut client_b, &mut vault);
    assert_eq!(balance_a, unwrap!(Coins::from_nano(7)));
    assert_eq!(balance_b, unwrap!(Coins::from_nano(3)));
}

#[test]
fn put_append_only_data() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();

    let mut client = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client, &mut vault);

    let start_nano = 1_000_000_000_000;
    let new_balance_owner = *client.public_id().public_key();
    common::perform_transaction(
        &mut env,
        &mut client,
        &mut vault,
        Request::CreateBalance {
            new_balance_owner,
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );

    // Seq
    let adata_name: XorName = env.rng().gen();
    let tag = 100;
    let mut adata = PubSeqAppendOnlyData::new(adata_name, tag);
    unwrap!(adata.append(&[(b"more".to_vec(), b"data".to_vec())], 0));
    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::PutAData(AData::PubSeq(adata)),
    );

    // Unseq
    let adata_name: XorName = env.rng().gen();
    let tag = 101;
    let mut adata = PubUnseqAppendOnlyData::new(adata_name, tag);
    unwrap!(adata.append(&[(b"more".to_vec(), b"data".to_vec())]));
    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::PutAData(AData::PubUnseq(adata)),
    );

    // Unpub Seq
    let adata_name: XorName = env.rng().gen();
    let tag = 102;
    let mut adata = UnpubSeqAppendOnlyData::new(adata_name, tag);
    unwrap!(adata.append(&[(b"more".to_vec(), b"data".to_vec())], 0));
    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::PutAData(AData::UnpubSeq(adata)),
    );

    // Unpub Unseq
    let adata_name: XorName = env.rng().gen();
    let tag = 103;
    let mut adata = UnpubUnseqAppendOnlyData::new(adata_name, tag);
    unwrap!(adata.append(&[(b"more".to_vec(), b"data".to_vec())]));
    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::PutAData(AData::UnpubUnseq(adata)),
    );

    // TODO - get the data to verify
}

#[test]
fn put_immutable_data() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();
    let conn_info = vault.connection_info();

    let mut client_a = TestClient::new(env.rng());
    let mut client_b = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client_a, &mut vault);
    common::establish_connection(&mut env, &mut client_b, &mut vault);

    let mut raw_data = vec![0u8; 1024];
    env.rng().fill(raw_data.as_mut_slice());
    let pub_idata = IData::Pub(PubImmutableData::new(raw_data.clone()));
    let unpub_idata = IData::Unpub(UnpubImmutableData::new(
        raw_data,
        *client_b.public_id().public_key(),
    ));

    // TODO - enable this once we're passed phase 1.
    if false {
        // Put should fail when the client has no associated balance.
        let message_id_1 =
            client_a.send_request(conn_info.clone(), Request::PutIData(pub_idata.clone()));
        let message_id_2 =
            client_b.send_request(conn_info.clone(), Request::PutIData(unpub_idata.clone()));
        env.poll(&mut vault);

        match client_a.expect_response(message_id_1) {
            Response::Mutation(Err(NdError::InsufficientBalance)) => (),
            x => unexpected!(x),
        }
        match client_b.expect_response(message_id_2) {
            Response::Mutation(Err(NdError::InsufficientBalance)) => (),
            x => unexpected!(x),
        }
    }

    // Create balances.  Client A starts with 2000 safecoins and spends 1000 to initialise
    // Client B's balance.
    let start_nano = 1_000_000_000_000;
    let message_id_1 = client_a.send_request(
        conn_info.clone(),
        Request::CreateBalance {
            new_balance_owner: *client_a.public_id().public_key(),
            amount: unwrap!(Coins::from_nano(2 * start_nano)),
            transaction_id: 0,
        },
    );
    let message_id_2 = client_a.send_request(
        conn_info.clone(),
        Request::CreateBalance {
            new_balance_owner: *client_b.public_id().public_key(),
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );
    env.poll(&mut vault);

    for message_id in &[message_id_1, message_id_2] {
        match client_a.expect_response(*message_id) {
            Response::Transaction(Ok(_)) => (),
            x => unexpected!(x),
        }
    }

    // Check client A can't Put an UnpubIData where B is the owner.
    let unpub_req = Request::PutIData(unpub_idata.clone());
    let mut message_id_1 = client_a.send_request(conn_info.clone(), unpub_req.clone());
    env.poll(&mut vault);
    match client_a.expect_response(message_id_1) {
        Response::Mutation(Err(NdError::InvalidOwners)) => (),
        x => unexpected!(x),
    }
    let mut balance_a = common::get_balance(&mut env, &mut client_a, &mut vault);
    let mut expected_balance = unwrap!(Coins::from_nano(start_nano));
    assert_eq!(expected_balance, balance_a);

    for _ in &[0, 1] {
        // Check they can both Put valid data.
        let pub_req = Request::PutIData(pub_idata.clone());
        message_id_1 = client_a.send_request(conn_info.clone(), pub_req);
        let mut message_id_2 = client_b.send_request(conn_info.clone(), unpub_req.clone());
        env.poll(&mut vault);

        match client_a.expect_response(message_id_1) {
            Response::Mutation(Ok(())) => (),
            x => unexpected!(x),
        }
        match client_b.expect_response(message_id_2) {
            Response::Mutation(Ok(())) => (),
            x => unexpected!(x),
        }
        balance_a = common::get_balance(&mut env, &mut client_a, &mut vault);
        let balance_b = common::get_balance(&mut env, &mut client_b, &mut vault);
        expected_balance = unwrap!(expected_balance.checked_sub(*COST_OF_PUT));
        assert_eq!(expected_balance, balance_a);
        assert_eq!(expected_balance, balance_b);

        // Check the data is retrievable.
        message_id_1 =
            client_a.send_request(conn_info.clone(), Request::GetIData(*pub_idata.address()));
        message_id_2 =
            client_b.send_request(conn_info.clone(), Request::GetIData(*unpub_idata.address()));
        env.poll(&mut vault);

        match client_a.expect_response(message_id_1) {
            Response::GetIData(Ok(received)) => assert_eq!(pub_idata, received),
            x => unexpected!(x),
        }
        match client_b.expect_response(message_id_2) {
            Response::GetIData(Ok(received)) => assert_eq!(unpub_idata, received),
            x => unexpected!(x),
        }
    }
}

#[test]
fn get_immutable_data_that_doesnt_exist() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();

    let mut client = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client, &mut vault);

    // Try to get non-existing published immutable data
    let address: XorName = env.rng().gen();
    common::send_request_expect_err(
        &mut env,
        &mut client,
        &mut vault,
        Request::GetIData(IDataAddress::Pub(address)),
        Response::GetIData(Err(NdError::NoSuchData)),
    );

    // Try to get non-existing unpublished immutable data while having no balance
    common::send_request_expect_err(
        &mut env,
        &mut client,
        &mut vault,
        Request::GetIData(IDataAddress::Unpub(address)),
        Response::GetIData(Err(NdError::AccessDenied)),
    );

    // Try to get non-existing unpublished immutable data while having balance
    let start_nano = 1_000_000_000_000;
    let new_balance_owner = *client.public_id().public_key();
    common::perform_transaction(
        &mut env,
        &mut client,
        &mut vault,
        Request::CreateBalance {
            new_balance_owner,
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );
    common::send_request_expect_err(
        &mut env,
        &mut client,
        &mut vault,
        Request::GetIData(IDataAddress::Unpub(address)),
        Response::GetIData(Err(NdError::NoSuchData)),
    );
}

#[test]
fn get_immutable_data_from_other_owner() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();

    let mut client_a = TestClient::new(env.rng());
    let mut client_b = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client_a, &mut vault);
    common::establish_connection(&mut env, &mut client_b, &mut vault);

    let start_nano = 1_000_000_000_000;
    let new_balance_owner = *client_a.public_id().public_key();
    common::perform_transaction(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::CreateBalance {
            new_balance_owner,
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );

    let start_nano = 1_000_000_000_000;
    let new_balance_owner = *client_b.public_id().public_key();
    common::perform_transaction(
        &mut env,
        &mut client_b,
        &mut vault,
        Request::CreateBalance {
            new_balance_owner,
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );

    // Client A uploads published data that Client B can fetch
    let raw_data = vec![1, 2, 3];
    let pub_idata = IData::Pub(PubImmutableData::new(raw_data.clone()));
    let pub_idata_address = *pub_idata.address();
    common::perform_mutation(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::PutIData(pub_idata),
    );
    assert_eq!(
        common::get_idata(&mut env, &mut client_a, &mut vault, pub_idata_address,),
        raw_data
    );
    assert_eq!(
        common::get_idata(&mut env, &mut client_b, &mut vault, pub_idata_address,),
        raw_data
    );

    // Client A uploads unpublished data that Client B can't fetch
    let raw_data = vec![42];
    let owner = client_a.public_id().public_key();
    let unpub_idata = IData::Unpub(UnpubImmutableData::new(raw_data.clone(), *owner));
    let unpub_idata_address = *unpub_idata.address();
    common::perform_mutation(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::PutIData(unpub_idata),
    );
    assert_eq!(
        common::get_idata(&mut env, &mut client_a, &mut vault, unpub_idata_address,),
        raw_data
    );
    common::send_request_expect_err(
        &mut env,
        &mut client_b,
        &mut vault,
        Request::GetIData(unpub_idata_address),
        Response::GetIData(Err(NdError::AccessDenied)),
    );
}

#[test]
fn put_pub_and_get_unpub_immutable_data_at_same_xor_name() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();

    let mut client = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client, &mut vault);

    // Create balance.
    let start_nano = 1_000_000_000_000;
    let new_balance_owner = *client.public_id().public_key();
    common::perform_transaction(
        &mut env,
        &mut client,
        &mut vault,
        Request::CreateBalance {
            new_balance_owner,
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );

    // Put and verify some published immutable data
    let raw_data = vec![1, 2, 3];
    let pub_idata = IData::Pub(PubImmutableData::new(raw_data.clone()));
    let pub_idata_address: XorName = *pub_idata.address().name();
    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::PutIData(pub_idata),
    );
    assert_eq!(
        common::get_idata(
            &mut env,
            &mut client,
            &mut vault,
            IDataAddress::Pub(pub_idata_address)
        ),
        raw_data
    );

    // Get some unpublished immutable data from the same address
    common::send_request_expect_err(
        &mut env,
        &mut client,
        &mut vault,
        Request::GetIData(IDataAddress::Unpub(pub_idata_address)),
        Response::GetIData(Err(NdError::NoSuchData)),
    );
}

#[test]
fn put_unpub_and_get_pub_immutable_data_at_same_xor_name() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();

    let mut client = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client, &mut vault);

    // Create balances.
    let start_nano = 1_000_000_000_000;
    let new_balance_owner = *client.public_id().public_key();
    common::perform_transaction(
        &mut env,
        &mut client,
        &mut vault,
        Request::CreateBalance {
            new_balance_owner,
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );

    // Put and verify some unpub immutable data
    let raw_data = vec![1, 2, 3];
    let owner = client.public_id().public_key();
    let unpub_idata = IData::Unpub(UnpubImmutableData::new(raw_data.clone(), *owner));
    let unpub_idata_address: XorName = *unpub_idata.address().name();
    common::perform_mutation(
        &mut env,
        &mut client,
        &mut vault,
        Request::PutIData(unpub_idata),
    );
    assert_eq!(
        common::get_idata(
            &mut env,
            &mut client,
            &mut vault,
            IDataAddress::Unpub(unpub_idata_address)
        ),
        raw_data
    );

    // Get some published immutable data from the same address
    common::send_request_expect_err(
        &mut env,
        &mut client,
        &mut vault,
        Request::GetIData(IDataAddress::Pub(unpub_idata_address)),
        Response::GetIData(Err(NdError::NoSuchData)),
    );
}

#[test]
fn delete_immutable_data_that_doesnt_exist() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();

    let mut client = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client, &mut vault);

    // Try to delete non-existing published idata while not having a balance
    let address: XorName = env.rng().gen();
    common::send_request_expect_err(
        &mut env,
        &mut client,
        &mut vault,
        Request::DeleteUnpubIData(IDataAddress::Pub(address)),
        Response::Mutation(Err(NdError::InvalidOperation)),
    );

    // Try to delete non-existing unpublished data while not having a balance
    common::send_request_expect_err(
        &mut env,
        &mut client,
        &mut vault,
        Request::GetIData(IDataAddress::Unpub(address)),
        Response::GetIData(Err(NdError::AccessDenied)),
    );

    // Try to delete non-existing unpublished data
    let start_nano = 1_000_000_000_000;
    let new_balance_owner = *client.public_id().public_key();
    common::perform_transaction(
        &mut env,
        &mut client,
        &mut vault,
        Request::CreateBalance {
            new_balance_owner,
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );
    common::send_request_expect_err(
        &mut env,
        &mut client,
        &mut vault,
        Request::GetIData(IDataAddress::Unpub(address)),
        Response::GetIData(Err(NdError::NoSuchData)),
    );
}

#[test]
fn delete_immutable_data() {
    let mut env = Environment::new();
    let mut vault = TestVault::new();

    let mut client_a = TestClient::new(env.rng());
    let mut client_b = TestClient::new(env.rng());
    common::establish_connection(&mut env, &mut client_a, &mut vault);
    common::establish_connection(&mut env, &mut client_b, &mut vault);

    let start_nano = 1_000_000_000_000;
    let new_balance_owner = *client_a.public_id().public_key();
    common::perform_transaction(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::CreateBalance {
            new_balance_owner,
            amount: unwrap!(Coins::from_nano(start_nano)),
            transaction_id: 0,
        },
    );

    let raw_data = vec![1, 2, 3];
    let pub_idata = IData::Pub(PubImmutableData::new(raw_data.clone()));
    let pub_idata_address: XorName = *pub_idata.address().name();
    common::perform_mutation(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::PutIData(pub_idata),
    );

    // Try to delete published data by constructing inconsistent Request
    common::send_request_expect_err(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::DeleteUnpubIData(IDataAddress::Pub(pub_idata_address)),
        Response::Mutation(Err(NdError::InvalidOperation)),
    );

    // Try to delete published data by raw XorName
    common::send_request_expect_err(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::DeleteUnpubIData(IDataAddress::Unpub(pub_idata_address)),
        Response::Mutation(Err(NdError::NoSuchData)),
    );

    let raw_data = vec![42];
    let owner = client_a.public_id().public_key();
    let unpub_idata = IData::Unpub(UnpubImmutableData::new(raw_data.clone(), *owner));
    let unpub_idata_address: XorName = *unpub_idata.address().name();
    common::perform_mutation(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::PutIData(unpub_idata),
    );

    // Delete unpublished data without being the owner
    common::send_request_expect_err(
        &mut env,
        &mut client_b,
        &mut vault,
        Request::DeleteUnpubIData(IDataAddress::Unpub(unpub_idata_address)),
        Response::Mutation(Err(NdError::AccessDenied)),
    );

    // Delete unpublished data without having the balance
    common::perform_mutation(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::DeleteUnpubIData(IDataAddress::Unpub(unpub_idata_address)),
    );

    // Delete unpublished data again
    common::send_request_expect_err(
        &mut env,
        &mut client_a,
        &mut vault,
        Request::DeleteUnpubIData(IDataAddress::Unpub(unpub_idata_address)),
        Response::Mutation(Err(NdError::NoSuchData)),
    )
}
