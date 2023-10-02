# Installing JsTz

Installation is still quite manual at this stage. Plus most of us started with all the right things installed to begin with. 
If anything doesn't work we'd love to hear from you. (Actually we'll hate it. We're only human and we're like fixing people's builds as much as anybody else, but we grudgingly accept it will make the product better so we'll at least try to feign enthusiasm).
Please have a look at our contributing and complaining section and let us know where you're having trouble. We'll try to give you a hand and update the documentation to make it easier for the next person.

## Install Nix
Nix is a package manager which allows you to create a sandboxed environment with all the right things installed to help you build what you need to get started. 
You can install it on most systems with one command, have a look at https://nix.dev/tutorials/install-nix and choose the recipe that matches you're operating system. 

## Clone and build the Tezos Repo
Traditionally blockchains don't allow floating points, the reasons for this are fairly technical and not very interesting, so we ignored them and made a couple of changes to the protocol. 
Unfortunately this means you need to check out our version of octez and build it from scratch. 


``` sh
git clone git@gitlab.com:tezos/tezos.git
cd tezos
git checkout eced5663816d95f314e44283eb0c058c63668a24
nix-shell --run make
```

I think the view from core is that we don't actually need this step, just get the octez binaries from somewhere.

## Clone and build Jstz
Now we're ready to build JsTz itself. We don't have a release branch just yet, but we're working on it.
Until then just build from main and hope for the best. 
#### Clone JsTz
``` sh
git clone git@github.com:trilitech/jstz.git
```
#### Copy the new Octez binaries where JsTz can find them.
``` sh
cp tezos/octez-node tezos/octez-client tezos/octez-smart-rollup-node jstz

```
#### Build JsTz
``` sh
nix-develop 
nix-shell --run "make build-installer"
cargo build --release --target wasm32-unknown-unknown

```
## Run the sandbox
There's no possible way that didn't work perfectly first time. But just to check we'll just run the sandbox to make sure everything's working.
``` sh
#eval $(./scripts/sandbox.sh )
```
This will spin up everything you need to start playing with JsTz. 
To check it's all gone smoothly we'll have a look at the logs

``` sh
tail -f logs/rollup/log
```
At first you'll just see a message `Waiting for node to activate...........`
After a while you should see something a bit like this

``` sh
Sep 29 17:00:17.807: Fetching 0 messages from block
Sep 29 17:00:17.807:   BLwG6tnpms81kCtqddc1yhiFvESBgxLY5i9XyHSpUN5A82CvuqB at level 43
Sep 29 17:00:17.809: Transitioned PVM at inbox level 43 to
Sep 29 17:00:17.809:   srs11afKWd1nKJymiEga83uuZ8ZAc39sUhaEkCk7mZXTb4tpKBgZDv at tick
Sep 29 17:00:17.809:   2211000000000 with 3 messages
Sep 29 17:00:17.811: Finished processing layer 1 head
Sep 29 17:00:17.811:   BLwG6tnpms81kCtqddc1yhiFvESBgxLY5i9XyHSpUN5A82CvuqB at level 43 in 5.58ms
Sep 29 17:00:17.811: Finished processing 1 layer 1 heads for levels 43 to 43
Sep 29 17:00:17.811: [tz1gjaF81ZRRv: publish, add_messages, cement, refute] injection
Sep 29 17:00:17.811:   Request pushed on 2023-09-29T16:00:17.811-00:00, treated in 1us, completed in 2us

```
If anything went wrong whe  the it's probably our fault not yours. 
You'll most likely see the `Waiting for node to activate...........` message continue until your whole screen is dots. If you have some experience with octez and want to try and fix it yourself that's awesome. If you succeed have a look at our contributing and complaining section and tell us what went wrong and what you did to fix it. 
If you can't fix it and you'd like some help then move the logs to a different directory (otherwise git will ignore them) and let us see your branch.
```sh
mv logs sandbox-logs
git checkout -b <your github name>@broken-build/
git add 
git commit -m "tell us what you think went wrong or just swear at us"
git push

```
We don't have much of a support section just yet so we can't make promises but hopefully one of the dev team will be able to help. 


If on the other hand it somehow worked correctly you're ready to build your first smart function.
