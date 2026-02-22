# payments_engine

A simple payments engine that ingests a CSV stream of client transactions and
produces a final account state per client.

## Overview

The binary expects a single CLI argument: the path to a CSV file containing
transactions with the following columns:

- `type` – `"deposit" | "withdrawal" | "dispute" | "resolve" | "chargeback"`
- `client` – `u16` client identifier
- `tx` – `u32` transaction identifier
- `amount` – decimal amount (only present for `deposit` / `withdrawal`)

Example invocation:


and ensure that no further state changes occur for locked accounts.

### How do I know the implementation is correct?

The core of the logic lives in the `Handler` type, which is thoroughly tested
with unit tests. The tests are written at the level of the stateful engine
(operating directly on `Handler`) rather than via the CSV entrypoint. This
gives deterministic coverage of the business rules independent of I/O.

The test suite includes:

- **Deposits**
    - Creating a new client and setting balances correctly.
    - Accumulating multiple deposits for the same client.
    - Ensuring a transaction record is inserted.
    - Ignoring deposits for locked clients and not creating transactions.

- **Withdrawals**
    - Successful withdrawals reduce `available` and insert a transaction.
    - Insufficient funds leave balances unchanged but still insert a transaction.
    - Ignoring withdrawals for locked clients (no balance change, no transaction).
    - Ignoring withdrawals for non-existent clients (no client created, no transaction).

- **Disputes**
    - Moving funds from `available` → `held` and marking the transaction disputed.
    - Ignoring disputes when:
        - The transaction does not exist,
        - The transaction belongs to another client,
        - The transaction is already disputed,
        - The client does not exist,
        - The client is locked.
    - Ensuring in all ignored cases that both client balances and transaction flags
      remain unchanged.

- **Resolves**
    - Moving funds from `held` → `available` and clearing the dispute flag.
    - Ignoring resolves when:
        - The transaction does not exist,
        - The transaction belongs to another client,
        - The transaction is not disputed,
        - The client does not exist,
        - The client is locked.
    - Verifying that balances and dispute status do not change in ignored scenarios.

- **Chargebacks**
    - Reducing `held`, locking the client, and clearing the dispute flag.
    - Ignoring chargebacks when:
        - The transaction does not exist,
        - The transaction belongs to another client,
        - The transaction is not disputed,
        - The client does not exist,
        - The client is already locked.
    - Verifying that balances, lock status, and dispute status remain unchanged
      for all ignored paths.

Each test constructs explicit handler and account states, invokes one operation,
and asserts on:

- `available`, `held`, and implied `total`,
- whether the account is locked,
- the presence and fields of the corresponding transaction record.

Because the tests cover both "happy path" and "ignored/edge" paths for each
operation, they collectively provide confidence that the implementation handles
all the supported cases as intended.

