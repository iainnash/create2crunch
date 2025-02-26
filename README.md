# create2crunch

> A Rust program for finding salts that create gas-efficient Ethereum addresses via CREATE2.

Provide three arguments: a factory address (or contract that will call CREATE2), a caller address (for factory addresses that require it as a protection against frontrunning), and the keccak-256 hash of the initialization code of the contract that the factory will deploy. (The example below references [`Create2Factory` on Ropsten](https://ropsten.etherscan.io/address/0xa779284f095ef2eBb8ee26cd8384e49C57b26996).)

```sh
$ git clone https://github.com/0age/create2crunch
$ cd create2crunch
$ export STARTS_WITH="1111111"
$ export FACTORY="0xa779284f095ef2eBb8ee26cd8384e49C57b26996"
$ export CALLER="<YOUR_ADDRESS_GOES_HERE>"
$ export INIT_CODE_HASH="<HASH_OF_YOUR_CONTRACT_INIT_CODE_GOES_HERE>"
$ cargo run --release $FACTORY $CALLER $INIT_CODE_HASH
```

## Mining for Specific Address Prefixes

You can search for addresses that start with a specific prefix:

```sh
$ cargo run --release $FACTORY $CALLER $INIT_CODE_HASH [GPU_DEVICE] [PREFIX]
```

For example, to find addresses that start with "1111111" using CPU:

```sh
$ cargo run --release $FACTORY $CALLER $INIT_CODE_HASH 255 $STARTS_WITH
```

Or to use GPU (device 0) for mining:

```sh
$ cargo run --release $FACTORY $CALLER $INIT_CODE_HASH 0 $STARTS_WITH
```

### Using the Immutable CREATE2 Factory

To find an address starting with a custom prefix using the Immutable CREATE2 Factory (0x0000000000FFe8B47B3e2130213B802212439497):

```sh
$ export PREFIX="cafe"
$ export CALLER="0x9444390c01Dd5b7249E53FAc31290F7dFF53450D"  # Replace with your address
$ export INIT_CODE_HASH="0x043e1feac23293ca2d8621097e676a60cf43513011db4a9e26d6f0e6445ae144"  # Replace with your contract's init code hash
$ cargo run --release 0x0000000000FFe8B47B3e2130213B802212439497 $CALLER $INIT_CODE_HASH 255 $PREFIX
```

For GPU mining (using device 0):

```sh
$ cargo run --release 0x0000000000FFe8B47B3e2130213B802212439497 $CALLER $INIT_CODE_HASH 0 $PREFIX
```

For each efficient address found, the salt, resultant addresses, and value _(i.e. approximate rarity)_ will be written to `efficient_addresses.txt`. Verify that one of the salts actually results in the intended address before getting in too deep - ideally, the CREATE2 factory will have a view method for checking what address you'll get for submitting a particular salt. Be sure not to change the factory address or the init code without first removing any existing data to prevent the two salt types from becoming commingled. There's also a _very_ simple monitoring tool available if you run `$python3 analysis.py` in another tab.

This tool was originally built for use with [`Pr000xy`](https://github.com/0age/Pr000xy), including with [`Create2Factory`](https://github.com/0age/Pr000xy/blob/master/contracts/Create2Factory.sol) directly.

There is also an experimental OpenCL feature that can be used to search for addresses using a GPU. To give it a try, include a fourth parameter specifying the device ID to use, and optionally a fifth and sixth parameter to filter returned results by a threshold based on leading zero bytes and total zero bytes, respectively. When using the prefix feature, set both thresholds to 0 to disable the zero-byte checks.

PRs welcome!
