use anyhow::{anyhow, Error, Result};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;
use std::{collections::HashMap, str::FromStr, sync::Arc};

use crate::chain_context::ChainContext;

pub struct TransactionService<T: ChainContext> {
    pub chain_context: Arc<T>,
}

impl<T: ChainContext> TransactionService<T> {
    pub fn new(chain_context: Arc<T>) -> Self {
        TransactionService { chain_context }
    }

    pub async fn create_transaction(
        &self,
        items: Arc<HashMap<String, HashMap<String, Decimal>>>,
    ) -> Result<Transaction> {
        if items.len() != 2 {
            return Err(Error::msg("Invalid number of users in trade state"));
        }
        let mut users = items.keys();
        let user1 = users.next().unwrap();
        let user2 = users.next().unwrap();
        let user1_offers = items.get(user1).unwrap();
        let user2_offers = items.get(user2).unwrap();

        let (offers1, offers2) = cancel_out_trade_tokens(user1_offers, user2_offers);

        if offers1.is_empty() && offers2.is_empty() {
            return Err(anyhow!("No point creating a transaction, no offers"));
        }
        let mut sender_atas: Vec<Pubkey> = vec![];
        let mut receiver_atas: Vec<Pubkey> = vec![];
        let mut token_mints: Vec<Pubkey> = vec![];
        let mut amounts: Vec<&Decimal> = vec![];

        for (token, amount) in &offers1 {
            let sender_ata =
                get_associated_token_address(&Pubkey::from_str(user1)?, &Pubkey::from_str(token)?);
            let receiver_ata =
                get_associated_token_address(&Pubkey::from_str(user2)?, &Pubkey::from_str(token)?);

            sender_atas.push(sender_ata);
            receiver_atas.push(receiver_ata);
            token_mints.push(Pubkey::from_str(token)?);
            amounts.push(amount);
        }
        for (token, amount) in &offers2 {
            let sender_ata =
                get_associated_token_address(&Pubkey::from_str(user2)?, &Pubkey::from_str(token)?);
            let receiver_ata =
                get_associated_token_address(&Pubkey::from_str(user1)?, &Pubkey::from_str(token)?);

            sender_atas.push(sender_ata);
            receiver_atas.push(receiver_ata);
            token_mints.push(Pubkey::from_str(token)?);
            amounts.push(amount);
        }

        // dbg!("Senders: {}", sender_atas.len());
        // dbg!("Receivers: {}", receiver_atas.len());
        // dbg!("Token mints: {}", token_mints.len());
        // dbg!("Amounts: {}", amounts.len());

        let mut accounts = vec![
            AccountMeta::new(Pubkey::from_str(user1)?, true),
            AccountMeta::new(Pubkey::from_str(user2)?, true),
        ];
        let remaining_accounts: Vec<AccountMeta> = [
            token_mints
                .iter()
                .map(|acc| AccountMeta::new_readonly(*acc, false))
                .collect::<Vec<AccountMeta>>(),
            [sender_atas, receiver_atas]
                .concat()
                .iter()
                .map(|acc| AccountMeta::new(*acc, false))
                .collect(),
        ]
        .concat();

        // dbg!("Remaining acocunts len: {}", remaining_accounts.len());
        // dbg!("Remaining acocunts: {}", &remaining_accounts);

        accounts.extend(remaining_accounts);
        // dbg!("All accounts len: {}", accounts.len());
        // dbg!("All accounts: {}", &accounts);

        let data = amounts.into_iter().flat_map(|d| d.serialize()).collect();

        let instruction = Instruction {
            program_id: self.chain_context.get_trade_with_me_program_id(),
            accounts,
            data,
        };

        let recent_blockhash = self.chain_context.get_latest_blockhash().await?;
        let mut tx = Transaction::new_with_payer(&[instruction], Some(&Pubkey::from_str(user1)?));
        tx.message.recent_blockhash = recent_blockhash;
        Ok(tx)
    }
}

fn cancel_out_trade_tokens(
    user1_offers: &HashMap<String, Decimal>,
    user2_offers: &HashMap<String, Decimal>,
) -> (HashMap<String, Decimal>, HashMap<String, Decimal>) {
    let mut offers1 = user1_offers.clone();
    let mut offers2 = user2_offers.clone();

    for (token, amount) in &mut offers1 {
        if let Some(amount2) = offers2.get_mut(token) {
            if amount2 > amount {
                *amount2 -= *amount;
                *amount = dec!(0.0);
            } else if amount2 < amount {
                *amount -= *amount2;
                *amount2 = dec!(0.0);
            } else {
                *amount = dec!(0.0);
                *amount2 = dec!(0.0);
            }
        }
    }
    offers1.retain(|_, amount| *amount > dec!(0.0));
    offers2.retain(|_, amount| *amount > dec!(0.0));

    (offers1, offers2)
}

#[cfg(test)]
mod test {
    use rust_decimal_macros::dec;

    use crate::chain_context::TestChainContext;

    use super::*;

    #[tokio::test]
    async fn should_create_transaction() {
        let user1 = Pubkey::new_unique().to_string();
        let user2 = Pubkey::new_unique().to_string();
        let token1 = Pubkey::new_unique().to_string();
        let token2 = Pubkey::new_unique().to_string();
        let token3 = Pubkey::new_unique().to_string();
        let token4 = Pubkey::new_unique().to_string();
        let token5 = Pubkey::new_unique().to_string();
        let token6 = Pubkey::new_unique().to_string();
        let token7 = Pubkey::new_unique().to_string();

        println!("User1: {}", &user1);
        println!("User2: {}", &user2);
        println!("Token1: {}", &token1);
        println!("Token2: {}", &token2);
        println!("Token3: {}", &token3);
        println!("Token4: {}", &token4);
        println!("Token5: {}", &token5);
        println!("Token6: {}", &token6);
        println!("Token7: {}", &token7);

        let user1_offers = HashMap::from([
            (token1, dec!(10.0)),
            (token2.clone(), dec!(3.5)),
            (token3, dec!(4.0)),
            (token6.clone(), dec!(4.0)),
            (token7.clone(), dec!(4.0)),
        ]);
        let user2_offers = HashMap::from([
            (token2, dec!(10.0)),
            (token4, dec!(1.0)),
            (token5, dec!(4.0)),
            (token6, dec!(4.0)),
            (token7, dec!(0.2)),
        ]);
        let items = HashMap::from([
            (user1, user1_offers),
            (user2, user2_offers)
        ]);
        let program_id= Pubkey::new_unique();
        println!("Program ID: {}", &program_id);

        let transaction_service = TransactionService::<TestChainContext>::new(Arc::new(TestChainContext{}));
        let tx = transaction_service.create_transaction(Arc::new(items)).await.unwrap();
        println!("Tx message: {:#?}", tx.message());

    }

    #[test]
    fn should_cancel_out_same_token_transfers() {
        let user1_offers = HashMap::from([
            ("token1".to_string(), dec!(10.0)),
            ("token2".to_string(), dec!(3.5)),
            ("token3".to_string(), dec!(4.0)),
            ("token6".to_string(), dec!(4.0)),
            ("token7".to_string(), dec!(4.0)),
        ]);
        let user2_offers = HashMap::from([
            ("token2".to_string(), dec!(10.0)),
            ("token4".to_string(), dec!(1.0)),
            ("token5".to_string(), dec!(4.0)),
            ("token6".to_string(), dec!(4.0)),
            ("token7".to_string(), dec!(0.2)),
        ]);
        let (offers1, offers2) = cancel_out_trade_tokens(&user1_offers, &user2_offers);

        assert_eq!(*offers1.get("token1").unwrap(), dec!(10.0));
        assert_eq!(offers1.get("token2"), None);
        assert_eq!(*offers1.get("token3").unwrap(), dec!(4.0));
        assert_eq!(*offers2.get("token2").unwrap(), dec!(6.5));
        assert_eq!(*offers2.get("token4").unwrap(), dec!(1.0));
        assert_eq!(*offers2.get("token5").unwrap(), dec!(4.0));
        assert_eq!(offers1.get("token6"), None);
        assert_eq!(offers2.get("token6"), None);
        assert_eq!(*offers1.get("token7").unwrap(), dec!(3.8));
        assert_eq!(offers2.get("token7"), None);
    }
}
