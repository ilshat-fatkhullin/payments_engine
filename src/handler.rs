use crate::types::{Client, Transaction};
use rust_decimal::Decimal;
use std::collections::HashMap;

pub struct Handler {
    clients: HashMap<u16, Client>,
    transactions: HashMap<u32, Transaction>,
}

impl Handler {
    pub fn new() -> Handler {
        Handler {
            clients: HashMap::new(),
            transactions: HashMap::new(),
        }
    }

    pub fn get_clients(&self) -> &HashMap<u16, Client> {
        &self.clients
    }

    pub fn deposit(&mut self, client_id: u16, tx: u32, amount: Decimal)
    {
        let client = self.clients.entry(client_id).or_default();
        if client.locked {
            return;
        }

        client.available += amount;
        self.transactions.insert(
            tx,
            Transaction {
                client: client_id,
                amount,
                disputed: false,
            },
        );
    }

    pub fn withdraw(&mut self, client_id: u16, tx: u32, amount: Decimal) {
        let client = match self.clients.get_mut(&client_id) {
            Some(c) => c,
            None => return,
        };
        if client.locked {
            return;
        }

        if client.available >= amount {
            client.available -= amount;
        }
        self.transactions.insert(
            tx,
            Transaction {
                client: client_id,
                amount,
                disputed: false,
            },
        );
    }

    pub fn dispute(&mut self, client_id: u16, tx: u32) {
        let amount = match self.transactions.get(&tx) {
            Some(tx) if tx.client == client_id && !tx.disputed => tx.amount,
            _ => return,
        };

        let client = match self.clients.get_mut(&client_id) {
            Some(c) => c,
            None => return,
        };
        if client.locked {
            return;
        }

        client.available -= amount;
        client.held += amount;

        if let Some(tx) = self.transactions.get_mut(&tx) {
            tx.disputed = true;
        }
    }

    pub fn resolve(&mut self, client_id: u16, tx: u32) {
        let amount = match self.transactions.get(&tx) {
            Some(tx) if tx.client == client_id && tx.disputed => tx.amount,
            _ => return,
        };

        let client = match self.clients.get_mut(&client_id) {
            Some(c) => c,
            None => return,
        };
        if client.locked {
            return;
        }

        client.held -= amount;
        client.available += amount;

        if let Some(tx) = self.transactions.get_mut(&tx) {
            tx.disputed = false;
        }
    }

    pub fn chargeback(&mut self, client_id: u16, tx: u32) {
        let amount = match self.transactions.get(&tx) {
            Some(tx) if tx.client == client_id && tx.disputed => tx.amount,
            _ => return,
        };

        let client = match self.clients.get_mut(&client_id) {
            Some(c) => c,
            None => return,
        };
        if client.locked {
            return;
        }

        client.held -= amount;
        client.locked = true;

        if let Some(tx) = self.transactions.get_mut(&tx) {
            tx.disputed = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn d(units: i64, scale: u32) -> Decimal {
        Decimal::new(units, scale)
    }

    #[test]
    fn deposit_creates_new_client_and_updates_balance() {
        let mut handler = Handler::new();
        let amount = d(1000, 2);

        handler.deposit(1, 10, amount);

        let clients = handler.get_clients();
        let client = clients.get(&1).expect("client should exist after deposit");
        assert_eq!(client.available, amount);
        assert_eq!(client.held, d(0, 0));
        assert!(!client.locked);
    }

    #[test]
    fn deposit_accumulates_to_existing_balance() {
        let mut handler = Handler::new();
        let first = d(1000, 2);
        let second = d(250, 2);

        handler.deposit(1, 1, first);
        handler.deposit(1, 2, second);

        let clients = handler.get_clients();
        let client = clients.get(&1).expect("client should exist after deposits");
        assert_eq!(client.available, first + second);
    }

    #[test]
    fn deposit_inserts_transaction_record() {
        let mut handler = Handler::new();
        let amount = d(500, 2);
        let client_id = 3;
        let tx_id = 42;

        handler.deposit(client_id, tx_id, amount);

        let tx = handler
            .transactions
            .get(&tx_id)
            .expect("transaction should be recorded");
        assert_eq!(tx.client, client_id);
        assert_eq!(tx.amount, amount);
        assert!(!tx.disputed);
    }

    #[test]
    fn deposit_is_ignored_for_locked_client() {
        let mut handler = Handler::new();
        let client_id = 5;

        handler.clients.insert(client_id, Client {
            available: d(1000, 2),
            held: d(0, 0),
            locked: true,
        });

        let before = handler
            .clients
            .get(&client_id)
            .unwrap()
            .available;

        handler.deposit(client_id, 7, d(500, 2));

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, before, "balance should not change for locked client");

        assert!(
            !handler.transactions.contains_key(&7),
            "no transaction should be created for locked client"
        );
    }

    #[test]
    fn withdraw_reduces_balance_and_records_transaction_when_sufficient_funds() {
        let mut handler = Handler::new();
        let client_id = 1;
        let initial = d(2000, 2);
        let withdraw_amount = d(750, 2);
        let tx_id = 11;

        handler.clients.insert(client_id, Client {
            available: initial,
            held: d(0, 0),
            locked: false,
        });

        handler.withdraw(client_id, tx_id, withdraw_amount);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(
            client.available,
            initial - withdraw_amount,
            "available balance should be reduced by withdrawal amount"
        );

        let tx = handler.transactions.get(&tx_id)
            .expect("withdrawal transaction should be recorded");
        assert_eq!(tx.client, client_id);
        assert_eq!(tx.amount, withdraw_amount);
        assert!(!tx.disputed);
    }

    #[test]
    fn withdraw_does_not_change_balance_when_insufficient_funds_but_records_transaction() {
        let mut handler = Handler::new();
        let client_id = 2;
        let initial = d(500, 2);
        let withdraw_amount = d(1000, 2);
        let tx_id = 12;

        handler.clients.insert(client_id, Client {
            available: initial,
            held: d(0, 0),
            locked: false,
        });

        handler.withdraw(client_id, tx_id, withdraw_amount);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(
            client.available,
            initial,
            "available balance should remain unchanged when funds are insufficient"
        );

        let tx = handler.transactions.get(&tx_id)
            .expect("transaction should still be recorded even if withdrawal fails");
        assert_eq!(tx.client, client_id);
        assert_eq!(tx.amount, withdraw_amount);
        assert!(!tx.disputed);
    }

    #[test]
    fn withdraw_is_ignored_for_locked_client_and_does_not_record_transaction() {
        let mut handler = Handler::new();
        let client_id = 3;
        let initial = d(1500, 2);
        let withdraw_amount = d(500, 2);
        let tx_id = 13;

        handler.clients.insert(client_id, Client {
            available: initial,
            held: d(0, 0),
            locked: true,
        });

        handler.withdraw(client_id, tx_id, withdraw_amount);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(
            client.available,
            initial,
            "balance should not change for locked client"
        );
        assert!(
            !handler.transactions.contains_key(&tx_id),
            "no transaction should be recorded for locked client"
        );
    }

    #[test]
    fn withdraw_is_ignored_for_nonexistent_client_and_does_not_record_transaction() {
        let mut handler = Handler::new();
        let client_id = 99;
        let withdraw_amount = d(1000, 2);
        let tx_id = 14;

        handler.withdraw(client_id, tx_id, withdraw_amount);

        assert!(
            handler.clients.get(&client_id).is_none(),
            "client should still not exist after withdraw on unknown client"
        );
        assert!(
            !handler.transactions.contains_key(&tx_id),
            "no transaction should be recorded when client does not exist"
        );
    }

    #[test]
    fn dispute_moves_funds_from_available_to_held_and_marks_transaction_disputed() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 21;
        let amount = d(1000, 2);

        handler.clients.insert(client_id, Client {
            available: amount,
            held: d(0, 0),
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: false,
        });

        handler.dispute(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, d(0, 0), "available should decrease by amount");
        assert_eq!(client.held, amount, "held should increase by amount");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(tx.disputed, "transaction should be marked as disputed");
    }

    #[test]
    fn dispute_is_ignored_if_transaction_does_not_exist() {
        let mut handler = Handler::new();
        let client_id = 1;

        handler.clients.insert(client_id, Client {
            available: d(1000, 2),
            held: d(0, 0),
            locked: false,
        });

        handler.dispute(client_id, 999);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, d(1000, 2));
        assert_eq!(client.held, d(0, 0));
    }

    #[test]
    fn dispute_is_ignored_if_transaction_belongs_to_different_client() {
        let mut handler = Handler::new();
        let client_id = 1;
        let other_client_id = 2;
        let tx_id = 22;
        let amount = d(500, 2);

        handler.clients.insert(client_id, Client {
            available: amount,
            held: d(0, 0),
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: other_client_id,
            amount,
            disputed: false,
        });

        handler.dispute(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, amount, "balance should not change");
        assert_eq!(client.held, d(0, 0));

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(!tx.disputed, "transaction should remain not disputed");
    }

    #[test]
    fn dispute_is_ignored_if_transaction_already_disputed() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 23;
        let amount = d(700, 2);

        handler.clients.insert(client_id, Client {
            available: amount,
            held: d(0, 0),
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: true,
        });

        handler.dispute(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, amount, "balance should not change");
        assert_eq!(client.held, d(0, 0), "held should not change");
    }

    #[test]
    fn dispute_is_ignored_if_client_does_not_exist() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 24;
        let amount = d(300, 2);

        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: false,
        });

        handler.dispute(client_id, tx_id);

        assert!(handler.clients.get(&client_id).is_none(), "client should not exist");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(!tx.disputed, "transaction should remain not disputed");
    }

    #[test]
    fn dispute_is_ignored_for_locked_client_and_does_not_mark_transaction_disputed() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 25;
        let amount = d(400, 2);

        handler.clients.insert(client_id, Client {
            available: amount,
            held: d(0, 0),
            locked: true,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: false,
        });

        handler.dispute(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, amount, "balance should not change");
        assert_eq!(client.held, d(0, 0), "held should not change");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(!tx.disputed, "transaction should not be marked disputed for locked client");
    }

    #[test]
    fn resolve_moves_funds_from_held_to_available_and_clears_dispute() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 31;
        let amount = d(1000, 2);

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: amount,
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: true,
        });

        handler.resolve(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.held, d(0, 0), "held should decrease by amount");
        assert_eq!(client.available, amount, "available should increase by amount");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(!tx.disputed, "transaction should be marked as not disputed");
    }

    #[test]
    fn resolve_is_ignored_if_transaction_does_not_exist() {
        let mut handler = Handler::new();
        let client_id = 1;

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: d(1000, 2),
            locked: false,
        });

        handler.resolve(client_id, 999);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, d(0, 0), "available should remain unchanged");
        assert_eq!(client.held, d(1000, 2), "held should remain unchanged");
    }

    #[test]
    fn resolve_is_ignored_if_transaction_belongs_to_different_client() {
        let mut handler = Handler::new();
        let client_id = 1;
        let other_client_id = 2;
        let tx_id = 32;
        let amount = d(500, 2);

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: amount,
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: other_client_id,
            amount,
            disputed: true,
        });

        handler.resolve(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, d(0, 0), "available should not change");
        assert_eq!(client.held, amount, "held should not change");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(tx.disputed, "transaction should remain disputed");
    }

    #[test]
    fn resolve_is_ignored_if_transaction_is_not_disputed() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 33;
        let amount = d(700, 2);

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: amount,
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: false,
        });

        handler.resolve(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, d(0, 0), "available should not change");
        assert_eq!(client.held, amount, "held should not change");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(!tx.disputed, "transaction should remain not disputed");
    }

    #[test]
    fn resolve_is_ignored_if_client_does_not_exist() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 34;
        let amount = d(300, 2);

        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: true,
        });

        handler.resolve(client_id, tx_id);

        assert!(handler.clients.get(&client_id).is_none(), "client should not exist");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(tx.disputed, "transaction should remain disputed");
    }

    #[test]
    fn resolve_is_ignored_for_locked_client_and_does_not_clear_dispute() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 35;
        let amount = d(400, 2);

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: amount,
            locked: true,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: true,
        });

        handler.resolve(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.available, d(0, 0), "available should not change");
        assert_eq!(client.held, amount, "held should not change");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(tx.disputed, "transaction should remain disputed for locked client");
    }

    #[test]
    fn chargeback_reduces_held_locks_account_and_clears_dispute() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 41;
        let amount = d(1000, 2);

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: amount,
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: true,
        });

        handler.chargeback(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.held, d(0, 0), "held should be reduced by disputed amount");
        assert!(client.locked, "client should be locked after chargeback");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(!tx.disputed, "transaction should no longer be disputed after chargeback");
    }

    #[test]
    fn chargeback_is_ignored_if_transaction_does_not_exist() {
        let mut handler = Handler::new();
        let client_id = 1;

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: d(1000, 2),
            locked: false,
        });

        handler.chargeback(client_id, 999);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.held, d(1000, 2), "held should remain unchanged");
        assert!(!client.locked, "client should remain unlocked");
    }

    #[test]
    fn chargeback_is_ignored_if_transaction_belongs_to_different_client() {
        let mut handler = Handler::new();
        let client_id = 1;
        let other_client_id = 2;
        let tx_id = 42;
        let amount = d(500, 2);

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: amount,
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: other_client_id,
            amount,
            disputed: true,
        });

        handler.chargeback(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.held, amount, "held should not change");
        assert!(!client.locked, "client should remain unlocked");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(tx.disputed, "transaction should remain disputed for other client");
    }

    #[test]
    fn chargeback_is_ignored_if_transaction_is_not_disputed() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 43;
        let amount = d(700, 2);

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: amount,
            locked: false,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: false,
        });

        handler.chargeback(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.held, amount, "held should not change");
        assert!(!client.locked, "client should remain unlocked");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(!tx.disputed, "transaction should remain not disputed");
    }

    #[test]
    fn chargeback_is_ignored_if_client_does_not_exist() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 44;
        let amount = d(300, 2);

        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: true,
        });

        handler.chargeback(client_id, tx_id);

        assert!(handler.clients.get(&client_id).is_none(), "client should not be created");
        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(tx.disputed, "transaction should remain disputed");
    }

    #[test]
    fn chargeback_is_ignored_for_locked_client_and_does_not_clear_dispute() {
        let mut handler = Handler::new();
        let client_id = 1;
        let tx_id = 45;
        let amount = d(400, 2);

        handler.clients.insert(client_id, Client {
            available: d(0, 0),
            held: amount,
            locked: true,
        });
        handler.transactions.insert(tx_id, Transaction {
            client: client_id,
            amount,
            disputed: true,
        });

        handler.chargeback(client_id, tx_id);

        let client = handler.clients.get(&client_id).unwrap();
        assert_eq!(client.held, amount, "held should not change for locked client");
        assert!(client.locked, "client should remain locked");

        let tx = handler.transactions.get(&tx_id).unwrap();
        assert!(tx.disputed, "transaction should remain disputed for locked client");
    }
}
