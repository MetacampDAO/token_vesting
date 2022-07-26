# Token Vesting Program

## The code allows you to
- Create vesting instructions for any SPL token
- Create unlock instructions
- Change the destination of the vested tokens
- Close vesting accounts

### Motivation
This contract heavily reference `Bonfida/token-vesting` contract.

Bonfida token-vesting program does not allow closing of accounts.
This contract allows the `employer` to cancel the contract and transfer the remaining amount back to the token source account.
