use anyhow::{Context, Result};
use bdk::bitcoin::util::bip32::ExtendedPrivKey;
use bdk::blockchain::ElectrumBlockchain;
use bdk::template::Bip84;
use bdk::wallet::wallet_name_from_descriptor;
use bdk::KeychainKind;
use bdk::{electrum_client, Wallet};
use bdk::{sled, SyncOptions};
use bip39::Mnemonic;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use hkdf::Hkdf;
use sha2::Sha256;
use std::path::PathBuf;
use std::str::FromStr;

fn main() -> Result<()> {
    let network = Network::Bitcoin;
    let electrum = "ssl://blockstream.info:700";

    let path = "path.seed";

    let bytes = std::fs::read(path)?;
    let mnemonic = Mnemonic::from_entropy(&bytes)?;
    let mut ext_priv_key_seed = [0u8; 64];
    Hkdf::<Sha256>::new(None, &mnemonic.to_seed_normalized(""))
        .expand(b"BITCOIN_WALLET_SEED", &mut ext_priv_key_seed)
        .expect("array is of correct length");

    let ext_priv_key = ExtendedPrivKey::new_master(network, &ext_priv_key_seed)?;
    let wallet_name = wallet_name(ext_priv_key, network)?;
    let db = database(wallet_name.as_str())?;
    let client = electrum_client::Client::new(electrum).unwrap();
    let blockchain = ElectrumBlockchain::from(client);

    let wallet = Wallet::new(
        Bip84(ext_priv_key, KeychainKind::External),
        Some(Bip84(ext_priv_key, KeychainKind::Internal)),
        network,
        db,
    )?;

    wallet.sync(&blockchain, SyncOptions::default())?;

    let balance = wallet.get_balance()?;

    println!("{balance}");

    let mut txs = wallet.list_transactions(false)?;

    txs.sort_by(|a, b| {
        a.confirmation_time
            .clone()
            .unwrap()
            .height
            .cmp(&b.clone().confirmation_time.unwrap().height)
    });

    for i in txs {
        println!("{i:?}");
    }

    Ok(())
}

fn wallet_name(xprv: ExtendedPrivKey, network: Network) -> Result<String> {
    wallet_name_from_descriptor(
        Bip84(xprv, KeychainKind::External),
        Some(Bip84(xprv, KeychainKind::Internal)),
        network,
        &Secp256k1::new(),
    )
    .context("Coould not create wallet name from descriptor")
}

fn database(wallet_name: &str) -> Result<sled::Tree> {
    let mut datadir = PathBuf::from_str("./")?;
    datadir.push("wallet-example");
    let database = sled::open(datadir).context("Could not open db")?;
    database
        .open_tree(wallet_name)
        .context("Could not open tree")
}
