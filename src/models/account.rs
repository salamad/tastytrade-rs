//! Account and customer models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::enums::{AuthorityLevel, MarginOrCash};

/// Customer profile information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct Customer {
    /// Customer's email address
    #[serde(default)]
    pub email: String,
    /// Customer's username (if available, also check `login` field)
    #[serde(default)]
    pub username: String,
    /// External identifier
    #[serde(default)]
    pub external_id: String,
    /// First name
    #[serde(default)]
    pub first_name: Option<String>,
    /// Last name
    #[serde(default)]
    pub last_name: Option<String>,
    /// Mobile phone number
    #[serde(default)]
    pub mobile_phone_number: Option<String>,
    /// Whether the customer has agreed to give up certain benefits
    #[serde(default)]
    pub agreed_to_margining: Option<bool>,
    /// Customer ID
    #[serde(default)]
    pub id: Option<String>,
    /// Prefixes for accounts
    #[serde(default)]
    pub prefixes: Option<Vec<String>>,
}

/// Trading account information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct Account {
    /// Unique account number
    #[serde(default)]
    pub account_number: String,
    /// External identifier for the account
    #[serde(default)]
    pub external_id: Option<String>,
    /// When the account was opened
    #[serde(default)]
    pub opened_at: Option<DateTime<Utc>>,
    /// User-defined nickname for the account
    #[serde(default)]
    pub nickname: Option<String>,
    /// Type of account (e.g., "Individual", "Joint")
    #[serde(default)]
    pub account_type_name: Option<String>,
    /// Whether account is flagged as pattern day trader
    #[serde(default)]
    pub day_trader_status: Option<bool>,
    /// Whether this is a margin or cash account
    #[serde(default)]
    pub margin_or_cash: Option<MarginOrCash>,
    /// Authority level for the current user
    #[serde(default)]
    pub authority_level: Option<AuthorityLevel>,
    /// Whether this is a foreign account
    #[serde(default)]
    pub is_foreign: Option<bool>,
    /// Whether this is a test/paper trading account
    #[serde(default)]
    pub is_test_drive: Option<bool>,
    /// Whether this account is closed
    #[serde(default)]
    pub is_closed: Option<bool>,
    /// Whether futures trading is enabled
    #[serde(default)]
    pub is_futures_approved: Option<bool>,
    /// Whether the account is funded
    #[serde(default)]
    pub is_firm_proprietary: Option<bool>,
    /// Whether the account is an IRA
    #[serde(default)]
    pub is_ira: Option<bool>,
    /// Investment objective
    #[serde(default)]
    pub investment_objective: Option<String>,
    /// Suitable options level
    #[serde(default)]
    pub suitable_options_level: Option<String>,
    /// When the account was created
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

impl Account {
    /// Get the account number as a strongly-typed value.
    pub fn account_number(&self) -> super::AccountNumber {
        super::AccountNumber::new(&self.account_number)
    }
}

/// Account item in the accounts list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountItem {
    /// The account details
    pub account: Account,
    /// Authority level for this account
    #[serde(default)]
    pub authority_level: Option<AuthorityLevel>,
}

/// Customer address information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Address {
    /// Street address line 1
    pub street_one: Option<String>,
    /// Street address line 2
    pub street_two: Option<String>,
    /// City
    pub city: Option<String>,
    /// State or province code
    pub state_region: Option<String>,
    /// Postal/ZIP code
    pub postal_code: Option<String>,
    /// Country code
    pub country: Option<String>,
    /// Whether this is a domestic address
    #[serde(default)]
    pub is_domestic: bool,
    /// Whether this is a foreign address
    #[serde(default)]
    pub is_foreign: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_account() {
        let json = r#"{
            "account-number": "5WV12345",
            "external-id": "ext-123",
            "opened-at": "2024-01-15T10:30:00Z",
            "account-type-name": "Individual",
            "day-trader-status": false,
            "is-foreign": false,
            "is-test-drive": true
        }"#;

        let account: Account = serde_json::from_str(json).unwrap();
        assert_eq!(account.account_number, "5WV12345");
        assert_eq!(account.account_type_name, Some("Individual".to_string()));
        assert_eq!(account.is_test_drive, Some(true));
    }
}
