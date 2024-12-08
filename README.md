# auto-verify-exex

## What is ExEx?

ExEx is a plugin system in Reth that allows users to define custom logic that gets
executed when certain events happen during the execution and syncing of the chain.

## Problem

Auto-verification on etherscan and family requires that the init code and the contract
code be constant. When you use immutables it embeds the immutables directly into the
deployed bytecode, and initcode has the constructor args, so neither of those things are
constants anymore. As a result auto-verification doesn't work. For making the auto
verification work people usually refrain from using immutables and instead have a oneoff
`initiatlize` function call that initializes the constants in the storage after the deployment. 
BUT, in doing so they are paying a huge gas cost of extra `SLOAD`(2100) instead of just a `PUSH`(3)
instruction. 700x more expensive for that one operation.

A prominent example of this is UniswapV2 factory.

```solidity
    function createPair(address tokenA, address tokenB) external returns (address pair) {
        require(tokenA != tokenB, 'UniswapV2: IDENTICAL_ADDRESSES');
        (address token0, address token1) = tokenA < tokenB ? (tokenA, tokenB) : (tokenB, tokenA);
        require(token0 != address(0), 'UniswapV2: ZERO_ADDRESS');
        require(getPair[token0][token1] == address(0), 'UniswapV2: PAIR_EXISTS'); // single check is sufficient
        bytes memory bytecode = type(UniswapV2Pair).creationCode;
        bytes32 salt = keccak256(abi.encodePacked(token0, token1));
        assembly {
            pair := create2(0, add(bytecode, 32), mload(bytecode), salt)
        }
        // initializing separately because they dont want to use constructor args and
        // immutables for auto verification to work!!!
        IUniswapV2Pair(pair).initialize(token0, token1);
        getPair[token0][token1] = pair;
        getPair[token1][token0] = pair; // populate mapping in the reverse direction
        allPairs.push(pair);
        emit PairCreated(token0, token1, pair, allPairs.length);
    }
```

Now, this would have led to significant gas savings because Uniswap is a very popular
contract and has gotten millions of transactions.

## Solution

We run an ExEx that will check all new transactions against a registry of [`Matcher`](./src/matcher.rs)
that contains the compiled bytecode, the constructor arg types, and the selector of the factory contract.

Whenever a match is found, it will automatically decode the constructor args and use the etherscan API
to verify the contract dynamically.
The registry can be public and updated by contributing a pull request. After that even if one person
runs this ExEx, it will verify contracts for everyone. Public good.

## Future work

- Use traces to capture internal calls and verify them too.
- Integrate the etherscan API to verify multi-file contracts, etc.
- Make it easier to just commit the source directly instead of the compiled bytecode.
