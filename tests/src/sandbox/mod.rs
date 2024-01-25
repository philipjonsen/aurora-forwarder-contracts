use near_sdk::serde_json::json;
use near_workspaces::types::NearToken;
use near_workspaces::{Account, AccountId, Contract, Worker};

pub mod aurora;
pub mod erc20;
pub mod forwarder;
pub mod fungible_token;

const AURORA_WASM: &[u8] = include_bytes!("../../../res/aurora-mainnet.wasm");
const FT_WASM: &[u8] = include_bytes!("../../../res/fungible-token.wasm");
const FORWARDER_WASM: &[u8] = include_bytes!("../../../res/aurora-forwarder.wasm");
const FEES_WASM: &[u8] = include_bytes!("../../../res/aurora-forward-fees.wasm");
const INIT_BALANCE_NEAR: u128 = 50;

pub struct Sandbox {
    worker: Worker<near_workspaces::network::Sandbox>,
    root_account: Account,
}

impl Sandbox {
    pub async fn new() -> anyhow::Result<Self> {
        let worker = near_workspaces::sandbox().await?;
        let root_account = worker.root_account()?;

        Ok(Self {
            worker,
            root_account,
        })
    }

    pub async fn create_subaccount(
        &self,
        name: &str,
        init_balance: u128,
    ) -> anyhow::Result<Account> {
        self.root_account
            .create_subaccount(name)
            .initial_balance(NearToken::from_near(init_balance))
            .transact()
            .await
            .map(|result| result.result)
            .map_err(Into::into)
    }

    pub async fn balance(&self, account_id: &AccountId) -> u128 {
        self.worker
            .view_account(account_id)
            .await
            .unwrap()
            .balance
            .as_yoctonear()
    }

    pub async fn deploy_ft(
        &self,
        total_supply: u128,
        name: &str,
        decimals: u8,
    ) -> anyhow::Result<(Contract, Account)> {
        let name_lower = name.to_lowercase();
        let owner_name = format!("{}-owner", &name_lower);
        let ft_owner_account = self
            .create_subaccount(&owner_name, INIT_BALANCE_NEAR)
            .await?;
        let ft_contract_account = self
            .create_subaccount(&name_lower, INIT_BALANCE_NEAR)
            .await?;
        let result = ft_contract_account.deploy(FT_WASM).await?;
        assert!(result.is_success());

        let contract = result.result;
        let result = contract
            .call("new")
            .args_json(json!({
                "owner_id": ft_owner_account.id(),
                "total_supply": total_supply.to_string(),
                "metadata": {
                    "spec": "ft-1.0.0",
                    "name": format!("Token {}", &name),
                    "symbol": name,
                    "decimals": decimals
                }
            }))
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success(), "{result:?}");

        Ok((contract, ft_owner_account))
    }

    pub async fn deploy_aurora(&self) -> anyhow::Result<Contract> {
        let aurora_account = self.create_subaccount("aurora", INIT_BALANCE_NEAR).await?;
        let result = aurora_account.deploy(AURORA_WASM).await?;
        assert!(result.is_success());
        let contract = result.result;
        let result = aurora_account
            .call(contract.id(), "new")
            .args_json(json!({
               "chain_id": 1_313_161_559,
                "owner_id": self.root_account.id(),
                "upgrade_delay_blocks": 0,
                "key_manager": self.root_account.id(),
                "initial_hashchain": null
            }))
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success());

        Ok(contract)
    }

    pub async fn deploy_forwarder(
        &self,
        target_network: &AccountId,
        address: &str,
    ) -> anyhow::Result<Contract> {
        let fwd_account = self
            .create_subaccount("forwarder", INIT_BALANCE_NEAR)
            .await?;
        let result = fwd_account.deploy(FORWARDER_WASM).await?;
        assert!(result.is_success());
        let contract = result.result;
        let result = fwd_account
            .call(contract.id(), "new")
            .args_json(json!({
                "target_address": address,
                "target_network": target_network
            }))
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success());

        Ok(contract)
    }

    pub async fn deploy_fee(&self) -> anyhow::Result<Contract> {
        let fee_account = self.create_subaccount("fees", INIT_BALANCE_NEAR).await?;
        let result = fee_account.deploy(FEES_WASM).await?;
        assert!(result.is_success());
        let contract = result.result;
        let result = fee_account
            .call(contract.id(), "new")
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success());

        Ok(contract)
    }
}
