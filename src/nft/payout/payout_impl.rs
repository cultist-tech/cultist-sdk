use crate::nft::{ NonFungibleToken, TokenId, refund_approved_account_ids };
use near_sdk::json_types::U128;
use crate::nft::payout::{ NonFungibleTokenPayout };
use crate::nft::royalty::{MINTER_ROYALTY_CAP, CONTRACT_ROYALTY_CAP, royalty_to_payout, Payout };
use near_sdk::{ env, assert_one_yocto, AccountId };
use std::collections::HashMap;
use crate::nft::events::NftTransferPayout;

impl NonFungibleTokenPayout for NonFungibleToken {
    fn nft_payout(&self, token_id: String, balance: U128, max_len_payout: u32) -> Payout {
        let owner_id = self.owner_by_id.get(&token_id).expect("No token");

        self.royalty.to_payout(balance, max_len_payout, token_id, owner_id)
    }

    fn nft_transfer_payout(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: u64,
        balance: U128,
        max_len_payout: u32,
        memo: Option<String>
    ) -> Payout {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        let (owner_id, approved_account_ids) = self.internal_transfer(
            &sender_id,
            &receiver_id,
            &token_id,
            Some(approval_id),
            memo
        );

        if let Some(approved_account_ids) = approved_account_ids {
            refund_approved_account_ids(owner_id.clone(), &approved_account_ids);
        }

        // compute payouts based on balance option
        // adds in contract_royalty and computes previous owner royalty from remainder
        let mut total_perpetual = 0;
        let balance_u128 = u128::from(balance);
        let mut payout: Payout = Payout { payout: HashMap::new() };
        let royalty = self.royalty.royalty_by_id.get(&token_id);

        if let Some(royalty) = royalty {
            assert!(
                (royalty.len() as u32) <= max_len_payout,
                "Market cannot payout to that many receivers"
            );

            for (k, v) in royalty.iter() {
                let key = k.clone();
                if key != owner_id {
                    payout.payout.insert(key, royalty_to_payout(*v, balance_u128));
                    total_perpetual += *v;
                }
            }
        }

        // payout to contract owner - may be previous token owner, they get remainder of balance
        if self.royalty.amount > 0 && self.royalty.receiver_id != owner_id {
            payout.payout.insert(
                self.royalty.receiver_id.clone(),
                royalty_to_payout(self.royalty.amount, balance_u128)
            );
            total_perpetual += self.royalty.amount;
        }
        assert!(
            total_perpetual <= MINTER_ROYALTY_CAP + CONTRACT_ROYALTY_CAP,
            "Royalties should not be more than caps"
        );
        // payout to previous owner
        payout.payout.insert(
            owner_id.clone(),
            royalty_to_payout(10000 - total_perpetual, balance_u128)
        );

        (NftTransferPayout {
            token_id: &token_id,
            sender_id: &sender_id,
            receiver_id: &receiver_id,
            balance: &balance,
            payout: &payout.payout,
        }).emit();

        payout
    }
}
