# create2crunch

Mines for an address with create2 with leading ones using the GPU.

```sh
$ git clone https://github.com/0age/create2crunch
$ cd create2crunch
$ export LEADING_ONES="1111111"
$ export FACTORY="0xa779284f095ef2eBb8ee26cd8384e49C57b26996"
$ export CALLER="<YOUR_ADDRESS_GOES_HERE>"
$ export INIT_CODE_HASH="<HASH_OF_YOUR_CONTRACT_INIT_CODE_GOES_HERE>"
$ cargo run --release $LEADING_ONES $FACTORY $CALLER $INIT_CODE_HASH 0
```
