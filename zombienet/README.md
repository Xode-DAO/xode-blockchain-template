# Zombienet (Development Platform)

ℹ️ Zombienet is a testing tool for the Polkadot and Substrate ecosystems. It allows developers to spin up and test local, multi-node networks (such as parachains or validators) in a fast and efficient way. 

## Steps
1. Download the polkadot, zombienet and xode binaries for your operating system.
2. Run chmod +x all binaries and script
3. Run cargo build --release at the root
4. Run ./zombienet-launch.sh
5. Open the appropriate link in polkadotJS

## Polkadot binaries
https://github.com/paritytech/polkadot/releases

## Compiling  Polkadot
```
git clone https://github.com/paritytech/polkadot
cd polkadot
git checkout release-v1.0.0
cargo build --release
```

## Zombienet binaries
https://github.com/paritytech/zombienet/releases

## Adding collator after launching Zombienet
### Generate a node key
```
../../polkadot/target/release/polkadot key generate-node-key 
```
### Output of generate-node-key (may change)
```
12D3KooWN5T5mGDMar9ZQfWs6JGpMVRBcVB135oxZUpsp5Y4bmMh
6fd556e9f3180c20d9d968e028f6bf97e9c962e57be6edf6143b3c48a172c388
```
### Run the node (use Zombienet chainspecs command)
- Copy the command generated by Zombienet
- Update the node-key
- Update the base-path  (Make sure it is empty)
- Update all rpc-ports (Make sure it does not conflict with the existing ports) 
```
../target/release/xode-node --name fredie --node-key 6fd556e9f3180c20d9d968e028f6bf97e9c962e57be6edf6143b3c48a172c388 --chain /var/folders/yl/9c9fgyj10wd29cbk4gc0bd7h0000gn/T/zombie-3af541275f25a65e6b6fbe244c2e4bf9_-15259-GnYSNiAYYb0G/dave/cfg/rococo-local-1000.json --base-path ./collator/data --rpc-cors all --unsafe-rpc-external --rpc-methods unsafe --rpc-port 9977 --collator -- --chain /var/folders/yl/9c9fgyj10wd29cbk4gc0bd7h0000gn/T/zombie-3af541275f25a65e6b6fbe244c2e4bf9_-15259-GnYSNiAYYb0G/dave/cfg/rococo-local.json --execution wasm --port 55877 --rpc-port 55878    
```
### Get the rotated keys of the parachain
```
curl http://127.0.0.1:9977 -H "Content-Type:application/json;charset=utf-8" -d '{ "jsonrpc":"2.0", "id":1,"method":"author_rotateKeys", "params": [] }'
```
### Output of the curl (may change)
```
{"jsonrpc":"2.0","id":1,"result":"0xa85644f08a9ad24e0f847afdef92113a8e36e286f817676764a856652d46556c"}
```
Developer->Extrinsics->Session->SetKeys (Proof: 0x).  Signed using the account owner of the node
![Screenshot 2024-11-28 at 9 35 47 AM](https://github.com/user-attachments/assets/9547ebfd-c282-4c1c-8958-f5ea09c6b506)



